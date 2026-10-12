#!/usr/bin/env python3
"""Generate independent synthetic train/dev data; never import acceptance fixtures."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import random
import string
import tempfile
import unicodedata


CATEGORIES = ("person", "organization", "location", "address", "event")
LANGUAGES = ("cs", "en", "de", "fr", "ru", "ja", "zh")
TEMPLATES = {
    "cs": "{person} zastupuje {organization}. Akce {event} se koná v místě {location}; kontaktní adresa je {address}.",
    "en": "{person} represents {organization}. The event {event} takes place in {location}; the contact address is {address}.",
    "de": "{person} vertritt {organization}. Die Veranstaltung {event} findet in {location} statt; die Kontaktadresse ist {address}.",
    "fr": "{person} représente {organization}. L'événement {event} a lieu à {location} ; l'adresse de contact est {address}.",
    "ru": "{person} представляет {organization}. Мероприятие {event} проходит в {location}; контактный адрес: {address}.",
    "ja": "{person}は{organization}の担当者です。{event}は{location}で開催され、連絡先住所は{address}です。",
    "zh": "{person}代表{organization}。活动{event}在{location}举行，联系地址是{address}。",
}
ROLES = {
    "cs": ("Kontaktní osoba", "Organizace", "Místo", "Adresa", "Událost"),
    "en": ("Contact person", "Organization", "Location", "Address", "Event"),
    "de": ("Kontaktperson", "Organisation", "Ort", "Adresse", "Veranstaltung"),
    "fr": ("Personne de contact", "Organisation", "Lieu", "Adresse", "Événement"),
    "ru": ("Контактное лицо", "Организация", "Место", "Адрес", "Мероприятие"),
    "ja": ("担当者", "組織", "場所", "住所", "イベント"),
    "zh": ("联系人", "组织", "地点", "地址", "活动"),
}
NEGATIVES = {
    "cs": "Testovací modul načetl matici. Tato věta neobsahuje žádnou konkrétní osobu ani organizaci.",
    "en": "The test module loaded a matrix. This sentence names no particular person or organization.",
    "de": "Das Testmodul hat eine Matrix geladen. Der Satz nennt keine bestimmte Person oder Organisation.",
    "fr": "Le module de test a chargé une matrice. La phrase ne nomme aucune personne ni organisation particulière.",
    "ru": "Тестовый модуль загрузил матрицу. Предложение не называет конкретного человека или организацию.",
    "ja": "テスト用モジュールが行列を読み込みました。この文は特定の人物や組織を示しません。",
    "zh": "测试模块已加载矩阵。这句话没有指明具体的人或组织。",
}
AMBIGUOUS = {
    "cs": "Výraz {person} nemá v tomto úryvku dostatečný kontext pro určení významu.",
    "en": "The expression {person} has insufficient context in this excerpt to determine its meaning.",
    "de": "Der Ausdruck {person} hat in diesem Ausschnitt zu wenig Kontext, um seine Bedeutung zu bestimmen.",
    "fr": "L'expression {person} manque de contexte dans cet extrait pour déterminer son sens.",
    "ru": "Для выражения {person} в этом отрывке недостаточно контекста, чтобы определить значение.",
    "ja": "この断片では{person}という表現の意味を判断するための文脈が不足しています。",
    "zh": "在这个片段中，表达式{person}缺乏足够的上下文来确定含义。",
}


def canonical(value):
    return " ".join(unicodedata.normalize("NFKC", value).casefold().split())


def word(rng, language):
    if language in ("ja", "zh"):
        alphabet = "澄霞岳梢紺奏遥翠凪蓮朔耀岑霄洛棠栖"
        return "".join(rng.choice(alphabet) for _ in range(rng.randint(3, 4)))
    consonants, vowels = ("бвгдклмнпрстф", "аеёиоуыэюя") if language == "ru" else ("bdklmnprstvz", "aeiou")
    if language == "cs":
        vowels = "aáeěiíoóuú"
    elif language == "de":
        vowels = "aeiouäöü"
    elif language == "fr":
        vowels = "aeiouéè"
    return "".join(rng.choice(consonants) + rng.choice(vowels) for _ in range(rng.randint(2, 3))).capitalize()


def new_surfaces(rng, language, used):
    while True:
        stems = [word(rng, language) for _ in CATEGORIES]
        given = word(rng, language)
        person = given + ("" if language in ("ja", "zh") else " ") + stems[0]
        number = rng.randint(17, 398)
        suffixes = {
            "cs": ("laboratoře", "Setkání", f"{stems[3]}ova {number}"),
            "en": ("Research", "Forum", f"{number} {stems[3]} Road"),
            "de": ("Forschung", "Forum", f"{stems[3]}straße {number}"),
            "fr": ("Recherche", "Forum", f"{number} rue {stems[3]}"),
            "ru": ("Исследования", "Форум", f"улица {stems[3]}, дом {number}"),
            "ja": ("研究所", "交流会", f"{stems[3]}通り{number}番地"),
            "zh": ("研究所", "交流会", f"{stems[3]}路{number}号"),
        }
        organization, event, address = suffixes[language]
        values = dict(zip(CATEGORIES, (person, f"{stems[1]} {organization}", stems[2], address, f"{event} {stems[4]}")))
        lexemes = {canonical(value) for value in [given, *stems]}
        normalized = lexemes | {canonical(value) for value in values.values()}
        if len(lexemes) == 6 and len({canonical(value) for value in values.values()}) == len(CATEGORIES) and normalized.isdisjoint(used):
            used.update(normalized)
            return values, normalized


def render(template, values):
    pieces, spans, expected, offset = [], [], [], 0
    for literal, field, spec, conversion in string.Formatter().parse(template):
        pieces.append(literal)
        offset += len(literal.encode("utf-8"))
        if field is not None:
            if field not in CATEGORIES or spec or conversion:
                raise ValueError("invalid synthetic template")
            value = values[field]
            size = len(value.encode("utf-8"))
            spans.append({"start": offset, "end": offset + size, "category": field})
            expected.append(value.encode("utf-8"))
            pieces.append(value)
            offset += size
    return "".join(pieces), spans, expected


def validate(row, expected):
    data = row["text"].encode("utf-8")
    if not data or len(data) > 1024 * 1024 or len(row["spans"]) != len(expected):
        raise ValueError("invalid synthetic row capacity")
    previous = 0
    for span, value in zip(row["spans"], expected):
        start, end = span["start"], span["end"]
        if span["category"] not in CATEGORIES or not previous <= start < end <= len(data):
            raise ValueError("invalid synthetic span")
        actual = data[start:end]
        if actual != value or actual.decode("utf-8").encode("utf-8") != actual:
            raise ValueError("synthetic annotation byte mismatch")
        data[:start].decode("utf-8")
        data[end:].decode("utf-8")
        previous = end
    if row["annotation_status"] == "review" and row["spans"]:
        raise ValueError("review examples must not claim gold annotations")


def generate(split, groups, seed, used):
    rng = random.Random(f"student-corpus-v1:{seed}:{split}")
    rows, split_surfaces, split_keys = [], set(), set()
    for language in LANGUAGES:
        for index in range(groups):
            group = f"student-v1-{split}-{language}-{index:05}"
            values, normalized = new_surfaces(rng, language, used)
            keys = [group] + [f"{category}:" + hashlib.sha256(canonical(value).encode()).hexdigest() for category, value in values.items()]
            keys.extend("lexeme:" + hashlib.sha256(value.encode()).hexdigest() for value in sorted(normalized))
            split_keys.update(keys)
            split_surfaces.update(normalized)
            variants = [("complete", TEMPLATES[language]), ("repeat", TEMPLATES[language] + "\n" + TEMPLATES[language]),
                        ("mixed", "🧪 «" + TEMPLATES[language] + "»\n" + TEMPLATES["en"])]
            variants.extend((category, role + ": {" + category + "}.") for category, role in zip(CATEGORIES, ROLES[language]))
            for family, template in variants:
                text, spans, expected = render(template, values)
                row = {"id": f"{group}-{family}", "split": split, "language": language, "group_id": group,
                       "group_keys": keys, "template_family": family, "annotation_status": "synthetic_gold", "text": text, "spans": spans}
                validate(row, expected)
                rows.append(row)
            noise = hashlib.sha256(f"{seed}:{group}".encode()).hexdigest()[:12]
            negative = NEGATIVES[language] + f" [test-vector=v{noise}]"
            for family, text, status in (("negative", negative, "synthetic_gold"),
                                         ("ambiguous", AMBIGUOUS[language].format(**values), "review")):
                row = {"id": f"{group}-{family}", "split": split, "language": language, "group_id": group,
                       "group_keys": keys, "template_family": family, "annotation_status": status, "text": text, "spans": []}
                validate(row, [])
                rows.append(row)
    return rows, split_surfaces, split_keys


def save(path, rows):
    data = "".join(json.dumps(row, ensure_ascii=False, separators=(",", ":")) + "\n" for row in rows).encode("utf-8")
    path.write_bytes(data)
    return {"rows": len(rows), "groups": len({row["group_id"] for row in rows}), "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(), "languages": dict(Counter(row["language"] for row in rows)),
            "categories": dict(Counter(span["category"] for row in rows for span in row["spans"])),
            "template_families": dict(Counter(row["template_family"] for row in rows)),
            "annotation_status": dict(Counter(row["annotation_status"] for row in rows))}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seed", type=int, default=731904)
    parser.add_argument("--train-groups-per-language", type=int, default=12)
    parser.add_argument("--dev-groups-per-language", type=int, default=4)
    args = parser.parse_args()
    if not 1 <= args.train_groups_per_language <= 1000 or not 1 <= args.dev_groups_per_language <= 1000:
        raise ValueError("group counts must be within 1..1000")
    output = args.output.resolve()
    if not any(output.is_relative_to(root.resolve()) for root in (Path("/tmp"), Path(tempfile.gettempdir()))):
        raise ValueError("synthetic corpus output must be a temporary directory")
    if output.exists():
        raise ValueError("synthetic corpus requires a fresh output directory")
    used = set()
    train, train_surfaces, train_keys = generate("train", args.train_groups_per_language, args.seed, used)
    dev, dev_surfaces, dev_keys = generate("dev", args.dev_groups_per_language, args.seed, used)
    if train_surfaces & dev_surfaces or train_keys & dev_keys:
        raise ValueError("synthetic train/dev leakage")
    if {canonical(row["text"]) for row in train} & {canonical(row["text"]) for row in dev}:
        raise ValueError("synthetic train/dev text leakage")
    if len({row["id"] for row in train + dev}) != len(train) + len(dev):
        raise ValueError("duplicate synthetic row id")
    output.mkdir(parents=True)
    manifest = {"schema": 1, "seed": args.seed, "synthetic_only": True, "existing_acceptance_fixtures_imported": False,
                "dev_generalization_proof": False, "flat_nonoverlapping_spans": True,
                "train_dev_surface_intersections": 0, "train_dev_group_key_intersections": 0,
                "train_dev_text_intersections": 0,
                "train": save(output / "train.jsonl", train), "dev": save(output / "dev.jsonl", dev)}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2))
    print(json.dumps({"train_rows": len(train), "dev_rows": len(dev), "train_dev_surface_intersections": 0}))


if __name__ == "__main__":
    main()
