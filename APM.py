"""Check empty-return appointments on APM Terminals (TERMPoint).

You sign in yourself in the browser window; this script never reads or types a
password. After you press Enter it checks each container and saves a
screenshot whenever no return appointment is offered.

    python APM.py MSMU1234567 TGHU7654321
"""

import argparse
import sys

from desk_browser import run_cli


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("containers", nargs="+", help="Container numbers to check")
    args = parser.parse_args()
    return run_cli("apm", args.containers)


if __name__ == "__main__":
    sys.exit(main())
