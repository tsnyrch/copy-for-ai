#!/usr/bin/env python3
"""Isolated model screening using synthetic UTF-8 fixtures only."""

import argparse
import json
from pathlib import Path
import resource
import time


SAMPLES = [
    ("cs", "Petr Dvořák pracuje pro Microsoft v Praze.", [("Petr Dvořák", "person"), ("Microsoft", "organization"), ("Praze", "location")]),
    ("en", "Alice Morgan works at Microsoft in London.", [("Alice Morgan", "person"), ("Microsoft", "organization"), ("London", "location")]),
    ("de", "Lena Fischer arbeitet bei Microsoft in Berlin.", [("Lena Fischer", "person"), ("Berlin", "location")]),
    ("fr", "Claire Dubois travaille chez Microsoft à Paris.", [("Claire Dubois", "person"), ("Paris", "location")]),
    ("ru", "Анна Петрова работает в Microsoft в Москве.", [("Анна Петрова", "person"), ("Москве", "location")]),
    ("ja", "田中太郎は東京でMicrosoftに勤務しています。", [("田中太郎", "person"), ("東京", "location")]),
    ("zh", "张伟在北京的Microsoft工作。", [("张伟", "person"), ("北京", "location")]),
    ("cs", "Eva Veselá a Adam Horák navštívili společnost Vega Systems v Litomyšli.", [("Eva Veselá", "person"), ("Adam Horák", "person"), ("Vega Systems", "organization"), ("Litomyšli", "location")]),
    ("cs", "Akce Petr Cup Litomyšl se koná v sobotu.", [("Petr Cup Litomyšl", "event")]),
    ("en", "Alice Morgan lives at 12 Baker Street, London.", [("Alice Morgan", "person"), ("12 Baker Street", "address")]),
    ("cs", "Petr Dvořák bydlí na adrese Dlouhá 12, Praha.", [("Petr Dvořák", "person"), ("Dlouhá 12", "address")]),
    ("de", "Lena Fischer wohnt in der Hauptstraße 12, Berlin.", [("Lena Fischer", "person"), ("Hauptstraße 12", "address")]),
    ("fr", "Claire Dubois habite au 10 rue de la Paix, Paris.", [("Claire Dubois", "person"), ("10 rue de la Paix", "address")]),
    ("ru", "Анна Петрова живет по адресу улица Тверская, дом 12, Москва.", [("Анна Петрова", "person"), ("улица Тверская, дом 12", "address")]),
    ("ja", "田中太郎の住所は東京都千代田区丸の内1-1です。", [("田中太郎", "person"), ("東京都千代田区丸の内1-1", "address")]),
    ("zh", "张伟住在北京市朝阳区建国路88号。", [("张伟", "person"), ("北京市朝阳区建国路88号", "address")]),
    ("cs", "Jana Novotná odpověděla Michalu Černému. Firma Orion Group připravuje konferenci.", [("Jana Novotná", "person"), ("Michalu Černému", "person"), ("Orion Group", "organization")]),
    ("cs", "Zápis: zákazník Jiří Šimek; dodavatel Alena Králová; místo Brno.", [("Jiří Šimek", "person"), ("Alena Králová", "person"), ("Brno", "location")]),
    ("en", "owner,company,city\nAlice Morgan,Microsoft,Paris\nClaire Dubois,Orion Group,London", [("Alice Morgan", "person"), ("Claire Dubois", "person"), ("Orion Group", "organization")]),
    ("cs", '{"Owner":"Eva Veselá","Vendor":"Vega Systems","Note":"návštěva Prahy"}', [("Eva Veselá", "person"), ("Vega Systems", "organization"), ("Prahy", "location")]),
    ("benign", "The team reviewed the report and prepared the next delivery.", []),
    ("benign", "Systém zpracoval požadavek. Výsledek je připraven ke kontrole.", []),
    ("benign", "The array has sixteen elements. The second element is empty.", []),
]


def fixture(text, expected, language):
    spans = []
    for value, category in expected:
        cursor = 0
        while (position := text.find(value, cursor)) >= 0:
            start = len(text[:position].encode())
            spans.append({"start": start, "end": start + len(value.encode()), "category": category})
            cursor = position + len(value)
    unique = {(span["start"], span["end"], span["category"]): span for span in spans}
    return {"language": language, "bytes": len(text.encode()), "gold": list(unique.values())}


def varied(size):
    paragraph = "Alice Morgan works at Microsoft in London. The team reviewed the report and prepared the next delivery. "
    tail = "\nPetr Dvořák pracuje pro Microsoft v Praze."
    pieces, length, index = [], 0, 0
    while length < size:
        piece = paragraph + f" reportref{index} "
        pieces.append(piece)
        length += len(piece)
        index += 1
    return "".join(pieces)[:size - len(tail.encode())] + tail


def prepare(directory):
    quality = directory / "quality"
    performance = directory / "performance"
    quality.mkdir(parents=True, exist_ok=True)
    performance.mkdir(parents=True, exist_ok=True)
    manifest = {}
    for index, (language, sample, expected) in enumerate(SAMPLES):
        for prefix in [0, 27, 31, 35]:
            text = "The team reviewed report. " * prefix + "\n" + sample + "\n" + sample
            case = f"case-{index:02}-{prefix:02}"
            (quality / f"{case}.txt").write_text(text)
            manifest[case] = fixture(text, expected, language)
    (quality / "manifest.json").write_text(json.dumps(manifest, indent=2))
    for size in [10240, 102400, 1048576]:
        text = varied(size)
        assert len(text.encode()) == size
        (performance / f"varied-{size}.txt").write_text(text)
    tail = "\n".join(sample for _, sample, _ in SAMPLES[:16])
    filler = "The team reviewed report. " * (1048576 // 26 + 1)
    text = filler[:1048576 - len(tail.encode())] + tail
    (quality / "tail.txt").write_text(text)
    manifest["tail"] = fixture(text, [(value, category) for _, _, values in SAMPLES[:16] for value, category in values], "multilingual-tail")
    (quality / "manifest.json").write_text(json.dumps(manifest, indent=2))


def parse_log(path, stage=None):
    rows = {}
    for line in path.read_text().splitlines():
        fields = dict(field.split("=", 1) for field in line.split() if "=" in field)
        if "case" not in fields:
            continue
        case = fields["case"]
        if line.startswith(("span ", "core_span ")):
            expected_prefix = "core_span " if stage == "core" else "span "
            if stage is not None and not line.startswith(expected_prefix):
                raise ValueError("range log stage mismatch")
            rows[case]["spans"].append({"start": int(fields["start"]), "end": int(fields["end"]), "category": fields["kind"].lower()})
        else:
            rows[case] = {**fields, "spans": []}
    return rows


def export_ranges(log_path, directory):
    directory.mkdir(parents=True, exist_ok=True)
    for case, row in parse_log(log_path).items():
        if Path(case).name != case:
            raise ValueError("invalid case id")
        (directory / f"{case}.tsv").write_text("".join(
            f"{span['start']}\t{span['end']}\t{span['category']}\n"
            for span in row["spans"]
        ))


def covered(text, gold, spans):
    cursor = gold["start"]
    data = text.encode()
    for span in sorted(spans, key=lambda value: value["start"]):
        if span["end"] <= cursor or span["start"] >= gold["end"]:
            continue
        gap = data[cursor:min(span["start"], gold["end"])].decode()
        if span["start"] > cursor and not all(character.isspace() for character in gap):
            return False
        cursor = max(cursor, span["end"])
        if cursor >= gold["end"]:
            return True
    return False


def summarize(directory, baseline_path, candidate_path, output, stage="ner"):
    manifest = json.loads((directory / "manifest.json").read_text())
    baseline, candidate = parse_log(baseline_path, stage), parse_log(candidate_path, stage)
    cases, total, lost, gained, base_miss, candidate_miss, benign = [], 0, 0, 0, 0, 0, {}
    for case, expected in manifest.items():
        text = (directory / f"{case}.txt").read_text()
        original, proposed = baseline[case]["spans"], candidate[case]["spans"]
        losses = []
        for gold in expected["gold"]:
            before, after = covered(text, gold, original), covered(text, gold, proposed)
            total += 1
            base_miss += not before
            candidate_miss += not after
            lost += before and not after
            gained += after and not before
            if before and not after:
                losses.append(gold)
        exact_before = {(span["start"], span["end"], span["category"]) for span in original}
        exact_after = {(span["start"], span["end"], span["category"]) for span in proposed}
        cases.append({"case": case, "language": expected["language"], "lost_masking": losses, "missing_ranges": len(exact_before - exact_after), "added_ranges": len(exact_after - exact_before)})
        if expected["language"] == "benign":
            # Compare only the annotated benign sample, excluding unannotated padding.
            sample = SAMPLES[int(case.split("-")[1])][1]
            start = len(text[:text.find(sample)].encode())
            end = len(text.encode())
            old_negative = {s for s in exact_before if s[1] > start and s[0] < end}
            new_negative = {s for s in exact_after if s[1] > start and s[0] < end}
            benign[case] = {"baseline": len(old_negative), "candidate": len(new_negative), "new_ranges": sorted(new_negative - old_negative), "removed_ranges": sorted(old_negative - new_negative)}
    benign_regression = sum(len(row["new_ranges"]) for row in benign.values())
    result = {"stage": stage, "gold": total, "baseline_uncovered": base_miss, "candidate_uncovered": candidate_miss, "lost_masking": lost, "gained_masking": gained, "benign_regression": benign_regression, "benign": benign, "cases": cases, "gate_pass": lost == 0 and candidate_miss <= base_miss and benign_regression == 0}
    output.write_text(json.dumps(result, indent=2))
    print(json.dumps({key: value for key, value in result.items() if key not in {"cases", "benign"}}))


def nym(args):
    from nym_probe import NymProbe
    model = NymProbe(args.model_dir, window_tokens=args.window)
    model.detect("The team reviewed the report.")
    with args.output.open("w") as log:
        for path in sorted(args.corpus.glob("*.txt")):
            text = path.read_text()
            start = time.perf_counter()
            spans = model.detect(text)
            categories = {"PERSON": "person", "ORG": "organization", "LOCATION": "location", "ADDRESS": "address"}
            for span in spans:
                span["category"] = categories[span["category"]]
            elapsed = time.perf_counter() - start
            metadata = model.last_metrics
            log.write(f"case={path.stem} bytes={len(text.encode())} detect_us={int(elapsed * 1000000)} windows={metadata['windows']} inferred={metadata['windows']} reused=0 entities={len(spans)} round_trip=pending\n")
            for span in spans:
                log.write(f"span case={path.stem} start={span['start']} end={span['end']} kind={span['category']}\n")
            log.flush()
            print(path.stem, int(elapsed * 1000), metadata, flush=True)
        log.write(f"peak_rss={resource.getrusage(resource.RUSAGE_SELF).ru_maxrss}\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="mode", required=True)
    prepare_parser = sub.add_parser("prepare")
    prepare_parser.add_argument("directory", type=Path)
    range_parser = sub.add_parser("ranges")
    range_parser.add_argument("log", type=Path)
    range_parser.add_argument("directory", type=Path)
    compare_parser = sub.add_parser("summarize")
    compare_parser.add_argument("corpus", type=Path)
    compare_parser.add_argument("baseline", type=Path)
    compare_parser.add_argument("candidate", type=Path)
    compare_parser.add_argument("output", type=Path)
    compare_parser.add_argument("--stage", choices=["ner", "core"], default="ner")
    nym_parser = sub.add_parser("nym")
    nym_parser.add_argument("--model-dir", type=Path, required=True)
    nym_parser.add_argument("--corpus", type=Path, required=True)
    nym_parser.add_argument("--output", type=Path, required=True)
    nym_parser.add_argument("--window", type=int, default=480)
    args = parser.parse_args()
    if args.mode == "prepare":
        prepare(args.directory)
    elif args.mode == "ranges":
        export_ranges(args.log, args.directory)
    elif args.mode == "summarize":
        summarize(args.corpus, args.baseline, args.candidate, args.output, args.stage)
    else:
        nym(args)


if __name__ == "__main__":
    main()
