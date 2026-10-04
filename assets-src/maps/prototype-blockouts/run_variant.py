"""PROTOTYPE, throwaway (#17). Blender entry point for one blockout variant.

  blender --background --factory-startup --python-exit-code 1 \
    --python run_variant.py -- <variant_key> <out_dir> [--no-render] [--quick]
"""
import importlib, os, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
argv = sys.argv[sys.argv.index("--") + 1:]
key, out_dir = argv[0], argv[1]
import blockout_lib  # noqa: E402

mod = importlib.import_module(f"variants.{key}")
m = mod.build()
m.finish(out_dir, render="--no-render" not in argv, quick="--quick" in argv)
