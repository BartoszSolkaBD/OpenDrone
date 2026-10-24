"""Throwaway test Map for #321 (never committed): the geometry demo with its
decals and blended ground, under baked light."""
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, "..", "..", "demos", "geometry"))
sys.path.insert(0, os.path.join(here, "..", "..", "lib"))
import build as demo  # noqa: E402

if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    builder, _cost = demo.build(argv[argv.index("--out") + 1], 0.15, light_cm=25, dressed=False)
    print("decal-plain:", builder.stats())
