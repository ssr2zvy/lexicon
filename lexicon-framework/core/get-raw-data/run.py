"""run.py — ONE function, ONE output: run a source's get-raw-data
implementation and return its exit code. This is the phase's single entry
point (exec'd by get-raw-data.sh; never bypassed). It owns ALL environment
setup — chdir to the source root and module loading — so impl files contain
nothing but their class and a SOURCE object."""

from __future__ import annotations

import importlib.util
import os
import sys

IMPL_REL = os.path.join("get-raw-data", "get_raw_data_impl.py")


def run(source_name: str, argv) -> int:
    here = os.path.dirname(os.path.abspath(__file__))
    root = os.path.dirname(os.path.dirname(here))          # lexicon-data/
    src_root = os.path.join(root, "sources", source_name)
    impl_path = os.path.join(src_root, IMPL_REL)
    if not os.path.isfile(impl_path):
        print(f"error: unknown source '{source_name}'; valid sources:",
              file=sys.stderr)
        for name in sorted(os.listdir(os.path.join(root, "sources"))):
            if os.path.isfile(os.path.join(root, "sources", name, IMPL_REL)):
                print(f"  {name}", file=sys.stderr)
        return 2
    os.chdir(src_root)   # BEFORE import: impl module-level code (throttle
    #                      paths etc.) relies on the source root as cwd
    spec = importlib.util.spec_from_file_location("get_raw_data_impl",
                                                  impl_path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    from get_raw_data import main   # this file's dir is sys.path[0]
    return main(mod.SOURCE, argv)


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("usage: run.py <source> [options...]", file=sys.stderr)
        sys.exit(2)
    sys.exit(run(sys.argv[1], sys.argv[2:]))
