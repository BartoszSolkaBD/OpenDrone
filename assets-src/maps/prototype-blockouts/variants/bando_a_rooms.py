"""PROTOTYPE (#17), Round 2. Bando A with brick rooms on the ground and first floors."""
from variants.bando_a import build as _build


def build():
    return _build(rooms=True)
