#!/usr/bin/env python
from openbabel import openbabel
import json
import sys
import os


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
    input_format,
    input_file,
    a, b,
    min_value,
    max_value
    ] = sys.argv[1:]

    conv = openbabel.OBConversion()
    conv.SetInFormat(input_format)
    mol = openbabel.OBMol()
    conv.ReadFile(mol, input_file)

    input_mapping = os.path.splitext(input_file)[0] + ".map.json"
    with open(input_mapping) as f:
        input_mapping = json.load(f)

    [[idx_a], [idx_b]] = [
        convert_name_to_index(name, input_mapping["ids"], input_mapping["groups"])
        for name in [a, b]
    ]

    min_value = float(min_value)
    max_value = float(max_value)

    atom_a = mol.GetAtomById(idx_a)
    atom_b = mol.GetAtomById(idx_b)

    distance = atom_a.GetDistance(atom_b)
    if distance < min_value:
        raise ValueError(f"Atoms get too close {distance}, {atom_a.GetType()}, {atom_b.GetType()}")
    if distance > max_value:
        raise ValueError(f"Atoms get to far away {distance}, {atom_a.GetType()}, {atom_b.GetType()}")

