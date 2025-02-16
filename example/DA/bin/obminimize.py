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
        ff_name,
        input_format,
        input_file,
        output_format,
        output_file,
        constraints_config,
        max_steps,
    ] = sys.argv[1:]

    conv = openbabel.OBConversion()
    conv.SetInAndOutFormats(input_format, output_format)
    mol = openbabel.OBMol()
    conv.ReadFile(mol, input_file)

    input_mapping = os.path.splitext(input_file)[0] + ".map.json"
    with open(input_mapping) as f:
        input_mapping = json.load(f)

    with open(constraints_config) as f:
        constraints_config = json.load(f)

    for k in ["ignore", "atom"]:
        v = constraints_config[k]
        v = [
            convert_name_to_index(name, input_mapping["ids"], input_mapping["groups"])
            for name in v
        ]
        v = [item + 1 for items in v for item in items]
        constraints_config[k] = v

    for index, [a, b, distance] in enumerate(constraints_config["distance"]):
        [[a], [b]] = [
            convert_name_to_index(name, input_mapping["ids"], input_mapping["groups"])
            for name in [a, b]
        ]
        constraints_config["distance"][index] = [a, b, distance]

    for index, [a, b, c, angle] in enumerate(constraints_config["angle"]):
        [[a], [b], [c]] = [
            convert_name_to_index(name, input_mapping["ids"], input_mapping["groups"])
            for name in [a, b, c]
        ]
        constraints_config["angle"][index] = [a, b, c, angle]

    for index, [a, b, c, d, torsion] in enumerate(constraints_config["torsion"]):
        [[a], [b], [c], [d]] = [
            convert_name_to_index(name, input_mapping["ids"], input_mapping["groups"])
            for name in [a, b, c, d]
        ]
        constraints_config["torsion"][index] = [a, b, c, d, torsion]

    constraints = openbabel.OBFFConstraints()
    for ignore in constraints_config["ignore"]:
        constraints.AddIgnore(ignore)
    for atom in constraints_config["atom"]:
        constraints.AddAtomConstraint(atom)
    for [a, b, distance] in constraints_config["distance"]:
        if distance is None:
            distance = mol.GetAtom(a).GetDistance(b)
        constraints.AddDistanceConstraint(a, b, float(distance))
    for [a, b, c, angle] in constraints_config["angle"]:
        if angle is None:
            angle = mol.GetAtom(a).GetAngle(b, c)
        constraints.AddAngleConstraint(a, b, c, float(angle))
    for [a, b, c, d, torsion] in constraints_config["torsion"]:
        if torsion is None:
            torsion = mol.GetTorsion(a, b, c, d)
        constraints.AddTorsionConstraint(a, b, c, d, float(torsion))

    ff = openbabel.OBForceField.FindForceField(ff_name)
    ff.Setup(mol, constraints)
    ff.SetConstraints(constraints)

    ff.ConjugateGradients(int(max_steps))
    ff.GetCoordinates(mol)

    # Write the mol to a file
    conv.WriteFile(mol, output_file)
