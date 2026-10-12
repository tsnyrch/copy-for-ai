"""Compact whole-text token preparation and deterministic BIO stitching."""

from dataclasses import dataclass
import hashlib
import struct

import numpy as np


_UINT32_MAX = np.iinfo(np.uint32).max
_WHITESPACE = np.array(
    [value for value in range(0x110000) if chr(value).isspace()], dtype=np.uint32
)


def _byte_prefix(text, raw):
    prefix = np.empty(len(text) + 1, dtype=np.uint32)
    source = np.frombuffer(raw, dtype=np.uint8)
    written = 0
    for start in range(0, len(raw), 1024 * 1024):
        positions = np.flatnonzero((source[start:start + 1024 * 1024] & 0xC0) != 0x80)
        count = len(positions)
        if written + count > len(text):
            raise ValueError("utf8_character_count_contract_failed")
        prefix[written:written + count] = positions + start
        written += count
    if written != len(text):
        raise ValueError("utf8_character_count_contract_failed")
    prefix[-1] = len(raw)
    return prefix


def _trim_offsets(text, offsets):
    characters = np.frombuffer(text.encode("utf-32-le"), dtype="<u4")
    whitespace = np.isin(characters, _WHITESPACE, assume_unique=False)
    del characters
    starts, ends = offsets[:, 0], offsets[:, 1]
    while True:
        active = np.flatnonzero(starts < ends)
        selected = active[whitespace[starts[active]]]
        if not len(selected):
            break
        starts[selected] += 1
    while True:
        active = np.flatnonzero(starts < ends)
        selected = active[whitespace[ends[active] - 1]]
        if not len(selected):
            break
        ends[selected] -= 1


@dataclass(frozen=True)
class PreparedText:
    ids: np.ndarray
    offsets: np.ndarray
    raw: bytes
    bucket: int
    overlap: int
    pad_id: int
    cls_id: int
    sep_id: int
    batch_size: int = 8

    @property
    def token_count(self):
        return len(self.ids)

    @property
    def window_count(self):
        if not self.token_count:
            return 0
        capacity = self.bucket - 2
        stride = capacity - self.overlap
        remaining = max(0, self.token_count - capacity)
        return 1 + (remaining + stride - 1) // stride

    @property
    def batch_count(self):
        return (self.window_count + self.batch_size - 1) // self.batch_size

    def batch(self, index, batch=None):
        batch = self.batch_size if batch is None else batch
        if not isinstance(index, int) or not isinstance(batch, int) or index < 0 or batch < 1:
            raise ValueError("invalid_batch_request")
        if batch != self.batch_size:
            raise ValueError("batch_differs_from_prepared_configuration")
        first = index * batch
        if first >= self.window_count:
            raise ValueError("batch_out_of_bounds")
        stride, capacity = self.bucket - 2 - self.overlap, self.bucket - 2
        windows = [
            (number * stride, min(number * stride + capacity, self.token_count))
            for number in range(first, min(first + batch, self.window_count))
        ]
        inputs = np.full((batch, self.bucket), self.pad_id, dtype=np.int32)
        masks = np.zeros((batch, self.bucket), dtype=np.int32)
        for row, (start, end) in enumerate(windows):
            length = end - start
            inputs[row, 0] = self.cls_id
            inputs[row, 1:length + 1] = self.ids[start:end]
            inputs[row, length + 1] = self.sep_id
            masks[row, :length + 2] = 1
        for row in range(len(windows), batch):
            inputs[row] = inputs[len(windows) - 1]
            masks[row] = masks[len(windows) - 1]
        inputs.setflags(write=False)
        masks.setflags(write=False)
        return inputs, masks, windows

    def stitch(self, choices, windows, labels, assigned):
        if (not isinstance(choices, np.ndarray) or choices.ndim != 2
                or choices.shape[1] != self.bucket or len(windows) > choices.shape[0]
                or choices.dtype.kind not in "iu" or np.any(choices > 10)
                or np.any(choices < 0)):
            raise ValueError("invalid_prediction_labels")
        if (labels.shape != (self.token_count,) or assigned.shape != labels.shape
                or labels.dtype != np.uint8 or assigned.dtype != np.uint8):
            raise ValueError("invalid_stitch_arrays")
        stride, capacity = self.bucket - 2 - self.overlap, self.bucket - 2
        for row, (start, end) in enumerate(windows):
            if (not 0 <= start < end <= self.token_count or start % stride
                    or start // stride >= self.window_count
                    or end != min(start + capacity, self.token_count)):
                raise ValueError("invalid_window_coordinates")
            left = start if start == 0 else start + self.overlap // 2
            right = end if end == self.token_count else end - (self.overlap - self.overlap // 2)
            if not left < right or np.any(assigned[left:right] != 0):
                raise ValueError("token_ownership_contract_failed")
            labels[left:right] = choices[row, 1 + left - start:1 + right - start]
            assigned[left:right] = 1

    def finish(self, labels, assigned):
        if (labels.shape != (self.token_count,) or assigned.shape != labels.shape
                or labels.dtype != np.uint8 or assigned.dtype != np.uint8
                or np.any(assigned != 1) or np.any(labels > 10)):
            raise ValueError("token_ownership_or_label_contract_failed")
        valid = np.flatnonzero(self.offsets[:, 0] < self.offsets[:, 1])
        selected = labels[valid]
        kinds = (selected.astype(np.int16) - 1) // 2
        begins = selected % 2 == 1
        previous = np.empty_like(kinds)
        following = np.empty_like(kinds)
        next_begin = np.empty_like(begins)
        if len(kinds):
            previous[0], previous[1:] = -1, kinds[:-1]
            following[-1], following[:-1] = -1, kinds[1:]
            next_begin[-1], next_begin[:-1] = False, begins[1:]
        opened = (kinds >= 0) & (begins | (previous != kinds))
        closed = (kinds >= 0) & (next_begin | (following != kinds))
        starts = self.offsets[valid[opened], 0]
        ends = self.offsets[valid[closed], 1]
        categories = kinds[opened]
        if (len(starts) != len(ends) or np.any(starts >= ends)
                or np.any(ends > len(self.raw)) or np.any(starts[1:] < ends[:-1])):
            raise ValueError("decoded_range_contract_failed")
        source = np.frombuffer(self.raw, dtype=np.uint8)
        if len(starts):
            internal_ends = ends[ends < len(source)]
            if (np.any((source[starts] & 0xC0) == 0x80)
                    or np.any((source[internal_ends] & 0xC0) == 0x80)):
                raise ValueError("decoded_utf8_contract_failed")
        digest = hashlib.sha256()
        for start, end, category in zip(starts, ends, categories, strict=True):
            digest.update(struct.pack("<QQB", int(start), int(end), int(category)))
        return {
            "decoded_span_count": len(starts),
            "span_sha256": digest.hexdigest(),
            "all_tokens_owned_once": True,
            "input_bytes": len(self.raw),
            "tokens": self.token_count,
            "valid_windows": self.window_count,
            "encoded_windows": self.batch_count * self.batch_size,
            "tail_covered": True,
            "last_token_end_byte": int(self.offsets[-1, 1]) if self.token_count else 0,
            "whitespace_empty_tokens_skipped": self.token_count - len(valid),
        }


def prepare(text, tokenizer, bucket=256, overlap=32, batch=8):
    if (not isinstance(bucket, int) or not isinstance(overlap, int)
            or not isinstance(batch, int) or batch < 1
            or bucket < 3 or not 0 <= overlap < bucket - 2):
        raise ValueError("invalid_window_configuration")
    try:
        raw = text.encode("utf-8")
    except UnicodeEncodeError:
        raise ValueError("input_utf8_contract_failed") from None
    if len(raw) > _UINT32_MAX:
        raise ValueError("input_exceeds_uint32_coordinates")
    encoded = tokenizer(text, add_special_tokens=False, return_offsets_mapping=True)
    ids = np.asarray(encoded["input_ids"], dtype=np.int64)
    offsets = np.asarray(encoded["offset_mapping"], dtype=np.int64).reshape(-1, 2)
    if (ids.ndim != 1 or len(ids) != len(offsets) or np.any(ids < 0)
            or np.any(ids > np.iinfo(np.int32).max) or np.any(offsets < 0)
            or np.any(offsets[:, 0] > offsets[:, 1]) or np.any(offsets > len(text))):
        raise ValueError("tokenizer_coordinate_contract_failed")
    ids = ids.astype(np.int32)
    offsets = offsets.astype(np.uint32)
    del encoded
    _trim_offsets(text, offsets)
    prefix = _byte_prefix(text, raw)
    offsets = prefix[offsets]
    ids.setflags(write=False)
    offsets.setflags(write=False)
    special = [tokenizer.pad_token_id, tokenizer.cls_token_id, tokenizer.sep_token_id]
    if any(not isinstance(value, int) or not 0 <= value <= np.iinfo(np.int32).max for value in special):
        raise ValueError("tokenizer_special_id_contract_failed")
    return PreparedText(ids, offsets, raw, bucket, overlap, *special, batch_size=batch)
