"""Painted brick with the plaster coming away: the same brick wall as
`brick_red_weathered`, 1.8 m to the picture, under a skim of grey-cream
plaster and a coat of faded mint-green paint. The paint has blistered and
come off in flakes where damp has got in; where the plaster has gone too the
brick shows. A faded safety band of chevrons runs across the wall, a scrawl of
generic graffiti has been sprayed over the paint and dripped, and rust-brown
water stains run from the joints."""

import numpy as np

import brickwork
import odtex as tex


def make(seed, n):
    s = tex.Seeds(seed + 2000)
    X, Y = tex.grid(n)
    fresh = brickwork.fresh_brick(X, Y, tex.Seeds(seed))
    joint = fresh["joint"]
    brick_albedo = fresh["albedo"]
    brick_height = fresh["height"]

    # Damp climbs from a few leaky places, and the paint comes away there.
    damp = tex.smoothstep(0.3, 0.75, tex.fbm(X, Y, (3, 2), 4, s()))
    amount = tex.clamp(0.1 + 0.5 * damp)
    warp_a = tex.perlin(X, Y, 5, s()) * 0.04
    warp_b = tex.perlin(X, Y, 5, s()) * 0.04
    flakes = tex.fbm(X + warp_a, Y + warp_b, 6, 5, s())
    paint_cut = 1.0 - amount
    gone_paint = tex.smoothstep(paint_cut - 0.006, paint_cut + 0.006, flakes)
    lip_paint = tex.smoothstep(paint_cut - 0.05, paint_cut - 0.006, flakes) * (1.0 - gone_paint)
    # The plaster is a thicker skim: it comes off in fewer, rougher pieces,
    # only where the paint has already gone.
    warp_c = tex.perlin(X, Y, 4, s()) * 0.05
    warp_d = tex.perlin(X, Y, 4, s()) * 0.05
    flakes2 = tex.fbm(X + warp_c, Y + warp_d, 4, 5, s())
    plaster_cut = 1.0 - amount * 0.8
    gone_plaster = tex.smoothstep(plaster_cut - 0.008, plaster_cut + 0.008, flakes2) * gone_paint
    lip_plaster = tex.smoothstep(plaster_cut - 0.05, plaster_cut - 0.008, flakes2) * (1.0 - gone_plaster) * gone_paint

    # Paint: mint green, uneven, chalky, with the stipple of a roller.
    fade = tex.fbm(X, Y, 6, 4, s())
    paint = tex.mix(tex.colour(112, 146, 128), tex.colour(166, 188, 168), fade)
    stipple = tex.fbm(X, Y, 180, 2, s())
    paint = paint * (0.93 + 0.14 * stipple)[..., None]
    chalk = tex.smoothstep(0.55, 0.8, tex.fbm(X, Y, 40, 3, s()))
    paint = tex.mix(paint, tex.colour(198, 206, 192), chalk * 0.4)
    # Blistering, in the damp places: little pale domes before the paint lifts.
    f1, _f2, _id, _dx, _dy = tex.voronoi(X, Y, 60, s())
    blister = tex.smoothstep(0.38, 0.12, f1) * tex.smoothstep(0.25, 0.6, damp) * (1.0 - gone_paint)
    blister = blister * tex.smoothstep(0.45, 0.6, tex.fbm(X, Y, 12, 3, s()))
    paint = tex.mix(paint, tex.colour(208, 216, 202), blister * 0.5)

    # Faded safety band: chevrons across the whole width of the wall.
    in_band = tex.band(Y, 0.46, 0.04, 0.002)
    chevrons = tex.sign_marks(X, Y, s(), (0.0, 0.42, 1.0, 0.5), "stripes")
    sun_fade = 0.4 + 0.5 * tex.fbm(X, Y, 12, 4, s())
    paint = tex.mix(paint, tex.colour(58, 60, 54), in_band * (1.0 - chevrons) * sun_fade * 0.7)
    paint = tex.mix(paint, tex.colour(196, 168, 66), chevrons * sun_fade * 0.9)

    # Graffiti: a loose scrawl in one red-orange, faded, with overspray and
    # its drips.
    tag = tex.scribbles(X, Y, s(), 3, 0.0045, (0.12, 0.08, 0.62, 0.34))
    haze = tex.blur(tag, 6, 2) * 2.5
    tag = tex.clamp(tag * (0.75 + 0.25 * tex.fbm(X, Y, 60, 3, s())) + haze * 0.12) * (1.0 - gone_paint)
    drips = tex.streaks(tex.smoothstep(0.3, 0.9, tag), 80, s(), X, Y, 160, 0.975)
    drips = drips * (1.0 - tex.smoothstep(0.2, 0.6, tag)) * 0.7
    spray = tex.clamp(tag + drips * (1.0 - gone_paint))
    paint = tex.mix(paint, tex.colour(198, 72, 40), spray * 0.8)

    # Plaster underneath: pale warm grey, crumbly.
    crumb = tex.fbm(X, Y, 50, 4, s())
    plaster = tex.mix(tex.colour(166, 158, 142), tex.colour(206, 198, 180), crumb)
    plaster = plaster * (0.9 + 0.2 * tex.fbm(X, Y, 14, 3, s()))[..., None]

    # Stack the layers: brick, then plaster, then paint.
    albedo = tex.mix(paint, plaster, gone_paint)
    albedo = tex.mix(albedo, brick_albedo * 0.78, gone_plaster)
    # Dark edge where paint or plaster ends, with the dirt that collects there.
    albedo = albedo * (1.0 - 0.3 * lip_paint[..., None] - 0.35 * lip_plaster[..., None])
    # Rust-brown water stains run down from the joints, and old damp marks.
    run = tex.streaks(joint * 0.8, 120, s(), X, Y, 36, 0.988)
    stain = tex.clamp(run * 1.3) * (1.0 - gone_plaster) * (0.35 + 0.65 * damp)
    albedo = tex.mix(albedo, albedo * tex.colour(176, 128, 92) * 1.2, stain * 0.55)
    albedo = albedo * (1.0 - 0.3 * damp * tex.fbm(X, Y, 8, 4, s()))[..., None]

    # Heights: paint on plaster on brick, each layer a step.
    brick_level = 0.15 + 0.35 * brick_height
    plaster_level = 0.62 + 0.1 * crumb
    paint_level = 0.72 + 0.05 * fade + 0.1 * blister + 0.03 * spray + 0.02 * stipple
    height = tex.mix(paint_level, plaster_level, gone_paint)
    height = tex.mix(height, brick_level, gone_plaster)
    # The brick's joints show faintly through the skim of plaster as a groove.
    height = height - joint * 0.07 * (1.0 - gone_plaster)
    height = tex.clamp(height + lip_paint * 0.05)

    rough = tex.mix(0.5 + 0.25 * chalk, 0.92, gone_paint)
    rough = tex.mix(rough, fresh["rough"], gone_plaster)
    rough = rough - spray * 0.1
    return tex.finish(albedo, height, rough, 7.0, seed=seed)
