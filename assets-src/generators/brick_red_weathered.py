"""Weathered red brick: old clinker bricks in a running bond, 1.8 m of wall to
the picture, as the weather has left them. Soot and rain streaks run down
from the joints, whole stretches are stained dark and some are wet, salt blooms
white on some bricks, moss greens the joints in the damp places, and one crack
steps down through the joints."""

import numpy as np

import brickwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 1000)
    X, Y = tex.grid(n)
    fresh = brickwork.fresh_brick(X, Y, tex.Seeds(seed))
    joint = fresh["joint"]
    edge = fresh["edge"]
    albedo = fresh["albedo"]

    # One crack steps down through the joints and across a brick or two.
    crack = tex.cracks(X, Y, 2, s(), 0.0035, 0.06, 0.0)
    near = tex.clamp(joint + 0.6 * tex.smoothstep(0.1, 0.0, edge))
    albedo = tex.mix(albedo, tex.colour(30, 22, 20), tex.clamp(crack * near * 1.5) * 0.9)

    # Where the water gathers: the recesses, and stretches the rain doesn't
    # wash.
    low = tex.cavity(1.0 - joint * 0.6, 3)
    patchy = tex.fbm(X, Y, (4, 3), 4, s())
    stain = tex.smoothstep(0.42, 0.7, patchy)
    # Rain runs down from every joint, soot-dark where it carries dirt.
    run = tex.streaks(joint * 0.9, 120, s(), X, Y, 52, 0.985)
    run2 = tex.streaks(tex.smoothstep(0.3, 0.8, tex.fbm(X, Y, 14, 3, s())) * joint, 200, s(), X, Y, 30, 0.992)
    dirt = tex.clamp(run * 0.7 + run2 * 0.9 + stain * 0.6 + low * 0.8)
    soot = tex.mix(tex.colour(52, 44, 42), tex.colour(78, 66, 60), tex.fbm(X, Y, 60, 3, s()))
    albedo = tex.mix(albedo, albedo * 0.38 + soot * 0.3, dirt * 0.85)

    # Wet stretches: darker, more saturated, glossier.
    wet = tex.smoothstep(0.55, 0.75, tex.fbm(X, Y, (3, 5), 4, s())) * (1.0 - joint)
    albedo = tex.mix(albedo, albedo * 0.62, wet * 0.7)

    # Salt: white bloom on the face of some bricks, and streaks under joints.
    salt = tex.smoothstep(0.6, 0.76, tex.fbm(X, Y, 20, 4, s())) * (1.0 - joint)
    weep = tex.streaks(joint * tex.smoothstep(0.5, 0.75, tex.fbm(X, Y, 10, 3, s())), 60, s(), X, Y, 70, 0.98)
    albedo = tex.mix(albedo, tex.colour(208, 204, 192), tex.clamp(salt * 0.3 + weep * 0.22))

    # Moss: in the joints and the lowest edge of the bricks, in damp patches.
    damp = tex.smoothstep(0.5, 0.68, tex.fbm(X, Y, (3, 3), 5, s()))
    moss_cover = damp * tex.clamp(joint * 0.8 + low * 0.7 + 0.1) * tex.smoothstep(0.35, 0.6, tex.fbm(X, Y, 40, 3, s()))
    moss_tone = tex.mix(tex.colour(52, 70, 30), tex.colour(92, 112, 44), tex.fbm(X, Y, 100, 3, s()))
    albedo = tex.mix(albedo, moss_tone, tex.clamp(moss_cover * 1.3))

    height = tex.clamp(fresh["height"] - crack * 0.35 + moss_cover * 0.06 + salt * 0.02)
    rough = fresh["rough"] - salt * 0.1 + moss_cover * 0.04 - dirt * 0.03 - wet * 0.25
    # The game's light is dimmer than a picture on a screen: a little brighter.
    return tex.finish(tex.clamp(albedo * 1.15), height, rough, 5.0, seed=seed)
