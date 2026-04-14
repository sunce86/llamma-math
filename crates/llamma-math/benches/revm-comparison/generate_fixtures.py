#!/usr/bin/env python3
"""Generate benchmark fixtures for llamma-math vs revm comparison.

Only saves bytecode + calldata + expected result.
Pool state is built by Rust adapter (Multicall3) at bench time.

Usage:
    RPC_URL=... python3 generate_fixtures.py
"""

import json, os, subprocess, re

RPC = os.environ["RPC_URL"]
FIXTURES_DIR = os.path.join(os.path.dirname(__file__), "fixtures")

def cc(addr, func, *args, block=None):
    cmd = ["cast", "call", addr, func] + list(args) + ["--rpc-url", RPC]
    if block: cmd += ["--block", str(block)]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=15)
    m = re.search(r'(-?\d+)', r.stdout.strip())
    return int(m.group(1)) if m else None

def gen(name, amm, dx, i=0, j=1):
    r = subprocess.run(["cast", "block-number", "--rpc-url", RPC],
                      capture_output=True, text=True, timeout=15)
    bn = int(r.stdout.strip()) - 5

    expected = cc(amm, "get_dy(uint256,uint256,uint256)(uint256)", str(i), str(j), str(dx), block=str(bn))
    if expected is None:
        print(f"  SKIP {name}: get_dy failed"); return

    calldata = "0x556d6e9f" + format(i, '064x') + format(j, '064x') + format(dx, '064x')

    fixture = {
        "name": name,
        "amm_address": amm,
        "calldata": calldata,
        "i": i, "j": j,
        "dx": str(dx),
        "expected_dy": str(expected),
        "block": bn,
    }

    path = os.path.join(FIXTURES_DIR, f"{name}.json")
    with open(path, "w") as f:
        json.dump(fixture, f, indent=2)
    print(f"  {name}: dy={expected} block={bn}")

if __name__ == "__main__":
    os.makedirs(FIXTURES_DIR, exist_ok=True)
    gen("weth_crvusd", "0x1681195C176239ac5E72d9aeBaCf5b2492E0C4ee", 100_000_000_000_000_000)
    gen("crv_llamalend", "0xafca625321Df8D6A068bDD8F1585d489D2acF11b", 1_000_000_000_000_000_000_000)
