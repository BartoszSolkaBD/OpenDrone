#!/usr/bin/env bash
# Downloads every clip listed in clips.toml into clips/ and verifies its SHA-256.
# The list below is generated from clips.toml; keep the two in step.
# Kenney packs are downloaded once and the members extracted; Freesound clips are
# the public HQ Ogg previews (no login needed).
set -euo pipefail
cd "$(dirname "$0")"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fetch() { # url out
  curl -fsSL --retry 3 --retry-delay 5 -o "$2" "$1"
}

sha_of() { shasum -a 256 "$1" | awk '{print $1}'; }

zip_for() { # url -> local path of the downloaded zip (downloaded once per run)
  local path
  path="$tmp/$(printf '%s' "$1" | shasum -a 256 | cut -c1-16).zip"
  if [ ! -f "$path" ]; then
    echo "fetching zip  $1" >&2
    fetch "$1" "$path"
  fi
  printf '%s\n' "$path"
}

clip() { # file sha256 url [zip_member]
  local file="$1" want="$2" url="$3" member="${4:-}" got zip
  if [ -f "$file" ] && [ "$(sha_of "$file")" = "$want" ]; then
    echo "ok (present)  $file"
    return
  fi
  mkdir -p "$(dirname "$file")"
  if [ -n "$member" ]; then
    zip="$(zip_for "$url")"
    unzip -p "$zip" "$member" > "$file.part"
  else
    fetch "$url" "$file.part"
  fi
  got="$(sha_of "$file.part")"
  if [ "$got" != "$want" ]; then
    echo "SHA-256 MISMATCH for $file" >&2
    echo "  source:   $url${member:+ :: $member}" >&2
    echo "  expected: $want" >&2
    echo "  got:      $got" >&2
    echo "  (bad download kept at $file.part)" >&2
    exit 1
  fi
  mv "$file.part" "$file"
  echo "ok  $file"
}

# hits
clip 'clips/hits/kenney-generic-light.ogg' 'f0e982611e97512fee5f777986b67e8b435434b601f94992ec044f7e89fb5acb' 'https://kenney.nl/media/pages/assets/impact-sounds/87b4ddecda-1677589768/kenney_impact-sounds.zip' 'Audio/impactGeneric_light_000.ogg'
clip 'clips/hits/kenney-plate-light.ogg' '6e797cf8a5bfc52d7ed5eb47c227974c1cf2a674954fec5b539cea7d62c7282c' 'https://kenney.nl/media/pages/assets/impact-sounds/87b4ddecda-1677589768/kenney_impact-sounds.zip' 'Audio/impactPlate_light_000.ogg'
clip 'clips/hits/kenney-soft-medium.ogg' '7d3ba0bb5e60a11b5d3e558c141303dcf494256675fbf753c0d252d2cf0481e3' 'https://kenney.nl/media/pages/assets/impact-sounds/87b4ddecda-1677589768/kenney_impact-sounds.zip' 'Audio/impactSoft_medium_000.ogg'
clip 'clips/hits/kenney-soft-heavy.ogg' '49e7ca88743fca974bb8676ea138b751cfd8f9033b5e7af8736c2a215d6edbc1' 'https://kenney.nl/media/pages/assets/impact-sounds/87b4ddecda-1677589768/kenney_impact-sounds.zip' 'Audio/impactSoft_heavy_000.ogg'
clip 'clips/hits/kenney-metal-light.ogg' '33b5e6e37c6e9d54e07bf5a89b12c76e879f40c1ea83cdd82714df1d6f9fec6d' 'https://kenney.nl/media/pages/assets/impact-sounds/87b4ddecda-1677589768/kenney_impact-sounds.zip' 'Audio/impactMetal_light_000.ogg'
clip 'clips/hits/kenney-metal-medium.ogg' 'a96f879fec0864a8938e0c745b6996a6c5679c16a234ce31a01cc995e8401003' 'https://kenney.nl/media/pages/assets/impact-sounds/87b4ddecda-1677589768/kenney_impact-sounds.zip' 'Audio/impactMetal_medium_000.ogg'
clip 'clips/hits/kenney-tin-medium.ogg' 'f4bbee66ca191ec744b84fc07ac9ed2a6fda955d51cfed8360f12b62815d3cbd' 'https://kenney.nl/media/pages/assets/impact-sounds/87b4ddecda-1677589768/kenney_impact-sounds.zip' 'Audio/impactTin_medium_000.ogg'
clip 'clips/hits/kenney-concrete-step.ogg' 'd7267e183067757c92c169de2a379abea592cfca6b39bb2e8feea15221ad79fe' 'https://kenney.nl/media/pages/assets/impact-sounds/87b4ddecda-1677589768/kenney_impact-sounds.zip' 'Audio/footstep_concrete_000.ogg'
clip 'clips/hits/kenney-stone-mining.ogg' '36b4ea107222d073c67ca64dde26944975609202d5090b2f2021213c1a7e35cd' 'https://kenney.nl/media/pages/assets/impact-sounds/87b4ddecda-1677589768/kenney_impact-sounds.zip' 'Audio/impactMining_000.ogg'
clip 'clips/hits/fs-quad-wall-collision.ogg' '54502c2bfd70429afdd822ef05007017ea9d4c1a986b6798c65c68dcbf9b5539' 'https://cdn.freesound.org/previews/854/854351_71257-hq.ogg'
clip 'clips/hits/fs-ardrone-prop-strike.ogg' 'a99a5b043bdf522d0d3db99830ac0c1d80c66856606722b50fcd904dc0269ba8' 'https://cdn.freesound.org/previews/332/332036_266678-hq.ogg'
clip 'clips/hits/fs-rc-car-metal-rail.ogg' 'af48d2804f5417aa3083b0e3b337ea2e9c957c0a440efa1b3f50ff90a13b8925' 'https://cdn.freesound.org/previews/726/726486_11865776-hq.ogg'
clip 'clips/hits/fs-small-plastic-breaks.ogg' '01440e44f8284bc55bf1cf71ca847df01f0577a5cf60785486896326dea1e1fd' 'https://cdn.freesound.org/previews/504/504585_1035118-hq.ogg'
# menus
clip 'clips/menus/kenney-click.ogg' 'ccfb7fa0cccdd9faec0eb16033c732b1e308d139d80f799161495d58f7adcdb9' 'https://kenney.nl/media/pages/assets/interface-sounds/fa43c1dd4d-1677589452/kenney_interface-sounds.zip' 'Audio/click_001.ogg'
clip 'clips/menus/kenney-select.ogg' 'aec0c31ea934a35936ae0d2ab8fac8123c93aa5647f935853a58dbaf90278b7a' 'https://kenney.nl/media/pages/assets/interface-sounds/fa43c1dd4d-1677589452/kenney_interface-sounds.zip' 'Audio/select_001.ogg'
clip 'clips/menus/kenney-back.ogg' '07db973f79f6ae0f2edc34561e7592e24d0577455919fb602cb8ecc0da991dcf' 'https://kenney.nl/media/pages/assets/interface-sounds/fa43c1dd4d-1677589452/kenney_interface-sounds.zip' 'Audio/back_001.ogg'
clip 'clips/menus/kenney-close.ogg' '44af8249b933e0fd35ec957a638bb1a0b01f85b53fbe9674bf77bd3ca3168ef4' 'https://kenney.nl/media/pages/assets/interface-sounds/fa43c1dd4d-1677589452/kenney_interface-sounds.zip' 'Audio/close_001.ogg'
clip 'clips/menus/kenney-toggle.ogg' 'ca1d2dde5f0b286abac4f070e23edab8f30927c8a533665cdf2ac6492a415e49' 'https://kenney.nl/media/pages/assets/interface-sounds/fa43c1dd4d-1677589452/kenney_interface-sounds.zip' 'Audio/toggle_001.ogg'
clip 'clips/menus/kenney-confirm.ogg' '063564703b6094d70718a3e787a55cc9141611e4ecd6b6637f8828f79b4a8c3a' 'https://kenney.nl/media/pages/assets/interface-sounds/fa43c1dd4d-1677589452/kenney_interface-sounds.zip' 'Audio/confirmation_001.ogg'
clip 'clips/menus/kenney-error.ogg' '46e67425d16339772e8d328fb36a49426c9467418686e11beeb71ff84b0f6433' 'https://kenney.nl/media/pages/assets/interface-sounds/fa43c1dd4d-1677589452/kenney_interface-sounds.zip' 'Audio/error_001.ogg'
# background-skate-park
clip 'clips/background-skate-park/fs-city-birds-distant-vehicles.ogg' '579da316aef144f1fca7ef536b67c56ae7bd6eaf850ce440137ffe165675f069' 'https://cdn.freesound.org/previews/640/640600_4897512-hq.ogg'
clip 'clips/background-skate-park/fs-urban-sunny-day.ogg' '4ccb3831c30a0e305d9a663c33b66a288fac6254193e9b4faa2bf905c0bdb541' 'https://cdn.freesound.org/previews/580/580646_12662124-hq.ogg'
clip 'clips/background-skate-park/fs-skate-park-street.ogg' '0551d5006f6ce645c64f8edf65d98d2002d41b2ce02a52fcce157bd4c8201f9e' 'https://cdn.freesound.org/previews/711/711196_14958854-hq.ogg'
# background-bando
clip 'clips/background-bando/fs-construction-site-wind-crane.ogg' '5c657af9e8d3a524054552ba8835a3386ee224ed34c0509230afbd042ef5b54b' 'https://cdn.freesound.org/previews/545/545035_3562198-hq.ogg'
clip 'clips/background-bando/fs-teufelsberg-wind-fabric.ogg' 'b844777b128cc1723d09dce9a696d562c30132c58a01256eb8fb1116eb034d72' 'https://cdn.freesound.org/previews/396/396674_255403-hq.ogg'
clip 'clips/background-bando/fs-city-from-high-building.ogg' '0e7298320fbe0b19a04236be8889634645e352816f6c6a794fc37f25b81a52cb' 'https://cdn.freesound.org/previews/640/640167_5471575-hq.ogg'

echo "All 26 clips present and verified."
