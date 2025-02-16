#!/usr/bin/env python
from openbabel import openbabel
import json
import sys


def convert_name_to_index(
    name: str, ids: dict[str, int], groups: dict[str, list[int]]
) -> list[int]:
    try:
        return [int(name)]
    except ValueError:
        value = groups.get(name)
        if value is None:
            value = ids.get(name)
            if value is None:
                raise ValueError(f"No name {name} found in both groups and ids record")
            return [value]
        return value


if __name__ == "__main__":
    [
        input_file,
        pattern,
        split,
        group_name,
        mapping_file,
        starts_from_1
    ] = sys.argv[1:]

    starts_from_1 = starts_from_1 == "true"

    with open(input_file, encoding="utf-8") as f:
        input_content = f.read()

    with open(mapping_file) as f:
        mapping_file = json.load(f)
    
    result = convert_name_to_index(group_name, mapping_file["ids"], mapping_file["groups"])

    result = split.join([str(value if not starts_from_1 else value + 1) for value in result])

    input_content = input_content.replace(pattern, result)

    with open(input_file, "w") as f:
        f.write(input_content)
