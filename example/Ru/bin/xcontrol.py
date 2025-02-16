#!/usr/bin/env python
import sys
import json
import numpy as np

if __name__ == "__main__":
    [json_name, x_control_name] = sys.argv[1:3]
    with open(json_name) as f:
        data = json.load(f)
    bonds = data["bonds"]
    bonds = [[a,b] for [a,b,_] in bonds]
    bonds = {
        i: [ a if a != i else b for [a,b] in bonds if a == i or b == i] for i in range(len(data["atoms"]))
    }
    with open(x_control_name, "w") as f:
        f.write("$ffnb\n")
        for idx in bonds:
            f.write(f"  nb = {idx + 1}: {", ".join([str(neighbor + 1) for neighbor in bonds[idx]])}\n")
        f.write("$end\n")


