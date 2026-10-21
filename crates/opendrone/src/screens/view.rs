//! Drawing the screens: a placeholder style on Bevy 0.20's UI and its
//! headless widgets (#68).
//!
//! The model ([`super::model`]) says what each screen holds; this draws it,
//! as plain dark panels in the game's default font, with one accent colour
//! for the item the cursor is on. Every size is in `rem`, so the Text size
//! setting scales the whole screen. Each item is a [`Button`] widget:
//! a click on it sends [`Activate`], and a moving mouse over it moves the
//! cursor. A screen is built again only when what it holds changes; the cursor
//! and the loading bar move in place.
//!
//! The look is a placeholder: the maintainer judges it by eye (#37 Testing
//! Decisions), and a later pass can restyle it without touching the model.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::picking::events::PointerMove;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button, observe};

use super::model::{Item, ItemId, ItemKind, Page, Screen};

const BACKDROP: Color = Color::srgb(0.07, 0.08, 0.11);
const OVERLAY: Color = Color::srgba(0.02, 0.03, 0.05, 0.72);
const PANEL: Color = Color::srgb(0.12, 0.14, 0.18);
const PANEL_SELECTED: Color = Color::srgb(0.20, 0.23, 0.30);
const ACCENT: Color = Color::srgb(1.0, 0.55, 0.0);
const TEXT: Color = Color::srgb(0.93, 0.94, 0.96);
const MUTED: Color = Color::srgb(0.62, 0.66, 0.74);
const WARNING: Color = Color::srgb(1.0, 0.78, 0.2);
const NOTICE: Color = Color::srgb(0.16, 0.30, 0.42);
const TRACK: Color = Color::srgb(0.18, 0.20, 0.25);

/// What a mouse did to the screen since the screens last looked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseAction {
    /// The mouse moved over an item.
    Point(ItemId),
    /// An item was clicked.
    Click(ItemId),
}

/// The mouse's doing, taken by the screens each frame.
#[derive(Resource, Default)]
pub struct MouseMenu(pub Vec<MouseAction>);

/// The screen as the model says it is now.
#[derive(Resource, Default)]
pub struct CurrentPage(pub Option<Page>);

/// What a screen's pictures are, loaded from the Pack's files once.
#[derive(Resource, Default)]
struct Pictures(BTreeMap<PathBuf, Option<Handle<Image>>>);

impl Pictures {
    /// A picture file as an image, `None` if it can't be read.
    fn get(&mut self, path: &Path, images: &mut Assets<Image>) -> Option<Handle<Image>> {
        self.0
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                let bytes = std::fs::read(path).ok()?;
                let image = Image::from_buffer(
                    &bytes,
                    ImageType::Extension("png"),
                    CompressedImageFormats::NONE,
                    true,
                    ImageSampler::linear(),
                    RenderAssetUsages::RENDER_WORLD,
                )
                .ok()?;
                Some(images.add(image))
            })
            .clone()
    }
}

/// The page that is built, with its cursor and bar taken out, so a change
/// to either doesn't build it again.
#[derive(Resource, Default)]
struct Built(Option<Page>);

/// The root of a built screen.
#[derive(Component)]
struct ScreenRoot;

/// An item on the screen.
#[derive(Component)]
struct ItemMarker {
    /// Its place in the page's items: where the cursor is when it's on it.
    index: usize,
}

/// The loading bar's filling.
#[derive(Component)]
struct LoadingBar;

/// The panel that scrolls when its content outgrows the window.
#[derive(Component)]
struct Scrolling;

/// Draws the model's page.
pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MouseMenu>()
            .init_resource::<CurrentPage>()
            .init_resource::<Pictures>()
            .init_resource::<Built>()
            .add_systems(
                Update,
                (
                    build_the_screen,
                    move_the_cursor,
                    keep_the_cursor_in_view,
                    fill_the_bar,
                )
                    .chain(),
            );
    }
}

/// A page without what moves in place.
fn shape_of(page: &Page) -> Page {
    let mut shape = page.clone();
    shape.cursor = 0;
    if let Some(loading) = &mut shape.loading {
        loading.progress = 0.0;
    }
    shape
}

fn build_the_screen(
    current: Res<CurrentPage>,
    mut built: ResMut<Built>,
    mut pictures: ResMut<Pictures>,
    mut images: ResMut<Assets<Image>>,
    roots: Query<Entity, With<ScreenRoot>>,
    mut commands: Commands,
) {
    let Some(page) = &current.0 else {
        return;
    };
    let shape = shape_of(page);
    if built.0.as_ref() == Some(&shape) {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    spawn_screen(&mut commands, page, &mut pictures, &mut images);
    built.0 = Some(shape);
}

/// The item under the cursor lights up, in place.
fn move_the_cursor(
    current: Res<CurrentPage>,
    mut items: Query<(&ItemMarker, &mut BackgroundColor, &mut BorderColor)>,
) {
    let Some(page) = &current.0 else {
        return;
    };
    for (item, mut background, mut border) in &mut items {
        let selected = item.index == page.cursor;
        let (wanted_background, wanted_border) = if selected {
            (PANEL_SELECTED, ACCENT)
        } else {
            (PANEL, Color::NONE)
        };
        if background.0 != wanted_background {
            background.0 = wanted_background;
        }
        if border.top != wanted_border {
            *border = BorderColor::all(wanted_border);
        }
    }
}

/// Scrolls the panel just far enough that the item under the cursor is in
/// view.
fn keep_the_cursor_in_view(
    current: Res<CurrentPage>,
    mut panels: Query<(&mut ScrollPosition, &ComputedNode, &UiGlobalTransform), With<Scrolling>>,
    items: Query<(&ItemMarker, &ComputedNode, &UiGlobalTransform)>,
) {
    let Some(page) = &current.0 else {
        return;
    };
    let Some((_, item, item_at)) = items.iter().find(|(m, _, _)| m.index == page.cursor) else {
        return;
    };
    for (mut scroll, panel, panel_at) in &mut panels {
        // Physical pixels, centres: the item's edges against the panel's.
        let item_top = item_at.translation.y - item.size().y / 2.0;
        let item_bottom = item_at.translation.y + item.size().y / 2.0;
        let panel_top = panel_at.translation.y - panel.size().y / 2.0;
        let panel_bottom = panel_at.translation.y + panel.size().y / 2.0;
        let moved = if item_top < panel_top {
            item_top - panel_top
        } else if item_bottom > panel_bottom {
            item_bottom - panel_bottom
        } else {
            continue;
        };
        scroll.y = (scroll.y + moved * panel.inverse_scale_factor).max(0.0);
    }
}

fn fill_the_bar(current: Res<CurrentPage>, mut bars: Query<&mut Node, With<LoadingBar>>) {
    let Some(loading) = current.0.as_ref().and_then(|p| p.loading.as_ref()) else {
        return;
    };
    for mut node in &mut bars {
        let wanted = percent(loading.progress * 100.0);
        if node.width != wanted {
            node.width = wanted;
        }
    }
}

fn text(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font_size: FontSize::Rem(size),
            ..default()
        },
        TextColor(color),
        // A line that is a name or a value stays on one line.
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
    )
}

fn paragraph(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font_size: FontSize::Rem(size),
            ..default()
        },
        TextColor(color),
        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
        Node {
            max_width: rem(34),
            ..default()
        },
    )
}

/// The ⚠ of a Pre-flight Warning, as a yellow badge: the placeholder font
/// has no ⚠ in it.
fn badge() -> impl Bundle {
    (
        Node {
            width: rem(1.3),
            height: rem(1.3),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(rem(0.3)),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(WARNING),
        children![text("!", 0.95, Color::BLACK)],
    )
}

fn warning_line(message: &str) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: rem(0.6),
            ..default()
        },
        children![badge(), paragraph(message, 0.9, WARNING)],
    )
}

fn spawn_screen(
    commands: &mut Commands,
    page: &Page,
    pictures: &mut Pictures,
    images: &mut Assets<Image>,
) {
    match page.screen {
        Screen::Flying => {
            if let Some(count) = page.countdown {
                commands.spawn((
                    ScreenRoot,
                    Node {
                        width: percent(100),
                        height: percent(100),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                    children![text(count.to_string(), 9.0, ACCENT)],
                ));
            }
        }
        Screen::Loading => spawn_loading(commands, page),
        _ => spawn_menu(commands, page, pictures, images),
    }
}

fn spawn_loading(commands: &mut Commands, page: &Page) {
    let Some(loading) = &page.loading else {
        return;
    };
    commands
        .spawn((
            ScreenRoot,
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(BACKDROP),
        ))
        .with_children(|root| {
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: rem(0.9),
                width: rem(34),
                ..default()
            })
            .with_children(|panel| {
                panel.spawn(text("Loading", 2.2, TEXT));
                panel.spawn(text(format!("Map: {}", loading.map), 1.1, TEXT));
                panel.spawn(text(format!("Quad: {}", loading.quad), 1.1, TEXT));
                panel.spawn(text(format!("Preset: {}", loading.preset), 1.1, TEXT));
                panel
                    .spawn((
                        Node {
                            width: percent(100),
                            height: rem(0.9),
                            border_radius: BorderRadius::all(rem(0.45)),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(TRACK),
                    ))
                    .with_children(|track| {
                        track.spawn((
                            LoadingBar,
                            Node {
                                width: percent(0),
                                height: percent(100),
                                ..default()
                            },
                            BackgroundColor(ACCENT),
                        ));
                    });
                panel.spawn(paragraph(
                    "Warming up the shaders, so the first flight doesn't stutter.",
                    0.9,
                    MUTED,
                ));
                panel
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(0.3),
                        margin: UiRect::top(rem(0.6)),
                        ..default()
                    })
                    .with_children(|keys| {
                        for hint in &loading.keys {
                            keys.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: rem(0.6),
                                ..default()
                            })
                            .with_children(|row| {
                                if hint.warning {
                                    row.spawn(badge());
                                }
                                let color = if hint.warning { WARNING } else { MUTED };
                                row.spawn(text(
                                    format!("{}: {}", hint.action, hint.key),
                                    0.95,
                                    color,
                                ));
                            });
                        }
                    });
            });
        });
}

fn spawn_menu(
    commands: &mut Commands,
    page: &Page,
    pictures: &mut Pictures,
    images: &mut Assets<Image>,
) {
    let background = if page.over_the_world {
        OVERLAY
    } else {
        BACKDROP
    };
    let hub = page.screen == Screen::Hub;
    let picker = matches!(page.screen, Screen::Picker(_));
    commands
        .spawn((
            ScreenRoot,
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(background),
        ))
        .with_children(|root| {
            // Short notices, top right.
            if !page.notices.is_empty() {
                root.spawn(Node {
                    position_type: PositionType::Absolute,
                    top: rem(1),
                    right: rem(1),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(0.4),
                    max_width: rem(24),
                    ..default()
                })
                .with_children(|stack| {
                    for notice in &page.notices {
                        stack
                            .spawn((
                                Node {
                                    padding: UiRect::all(rem(0.6)),
                                    border_radius: BorderRadius::all(rem(0.3)),
                                    ..default()
                                },
                                BackgroundColor(NOTICE),
                            ))
                            .with_child(paragraph(notice.clone(), 0.9, TEXT));
                    }
                });
            }
            // The panel scrolls when a bigger Text size or a small window
            // leaves it no room: the cursor's item is kept in view.
            root.spawn((
                Scrolling,
                ScrollPosition::default(),
                Node {
                    flex_direction: FlexDirection::Column,
                    width: if picker || hub { rem(40) } else { rem(34) },
                    max_height: percent(100),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
            ))
            .with_children(|scroll| {
                scroll
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(0.6),
                        width: percent(100),
                        padding: UiRect::vertical(rem(0.8)),
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|panel| {
                        panel.spawn(text(page.title.clone(), 2.2, TEXT));
                        if page.screen == Screen::FirstLaunch {
                            spawn_first_launch(panel, page, pictures, images);
                            return;
                        }
                        for line in &page.lines {
                            panel.spawn(paragraph(line.clone(), 1.0, MUTED));
                        }
                        if hub {
                            spawn_hub_items(panel, page, pictures, images);
                        } else {
                            for (index, item) in page.items.iter().enumerate() {
                                spawn_item(panel, item, index, pictures, images);
                            }
                        }
                        if let Some(flying_with) = &page.flying_with {
                            panel.spawn(text(flying_with.clone(), 0.95, MUTED));
                        }
                        for warning in &page.warnings {
                            panel.spawn(warning_line(warning));
                        }
                    });
            });
        });
}

/// First Launch, on one screen: Text size and Reduce motion side by side at
/// the top, the flashing-images notice, the question, and its answers.
fn spawn_first_launch(
    panel: &mut ChildSpawnerCommands<'_>,
    page: &Page,
    pictures: &mut Pictures,
    images: &mut Assets<Image>,
) {
    let (settings, answers) = page.items.split_at(page.items.len().min(2));
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: rem(0.6),
            ..default()
        })
        .with_children(|row| {
            for (index, item) in settings.iter().enumerate() {
                spawn_chip(row, item, index, pictures, images);
            }
        });
    if let Some(notice) = page.lines.first() {
        panel
            .spawn((
                Node {
                    padding: UiRect::all(rem(0.7)),
                    border_radius: BorderRadius::all(rem(0.4)),
                    ..default()
                },
                BackgroundColor(NOTICE),
            ))
            .with_child(paragraph(notice.clone(), 0.9, TEXT));
    }
    if let Some(question) = page.lines.get(1) {
        panel.spawn(text(question.clone(), 1.2, TEXT));
    }
    for (index, item) in answers.iter().enumerate() {
        spawn_item(panel, item, index + settings.len(), pictures, images);
    }
}

/// The Hub: FLY, then the Map, Quad and Preset chips side by side, then
/// Settings and Quit.
fn spawn_hub_items(
    panel: &mut ChildSpawnerCommands<'_>,
    page: &Page,
    pictures: &mut Pictures,
    images: &mut Assets<Image>,
) {
    let (fly, rest) = page
        .items
        .split_first()
        .map_or((None, &[][..]), |(f, r)| (Some(f), r));
    if let Some(fly) = fly {
        spawn_item(panel, fly, 0, pictures, images);
    }
    let (chips, buttons) = rest.split_at(rest.len().min(3));
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: rem(0.6),
            ..default()
        })
        .with_children(|row| {
            for (at, item) in chips.iter().enumerate() {
                spawn_chip(row, item, at + 1, pictures, images);
            }
        });
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: rem(0.6),
            ..default()
        })
        .with_children(|row| {
            for (at, item) in buttons.iter().enumerate() {
                spawn_item(row, item, at + 4, pictures, images);
            }
        });
}

/// A Hub chip: the setting's name over its value, with its picture if it has
/// one.
fn spawn_chip(
    row: &mut ChildSpawnerCommands<'_>,
    item: &Item,
    index: usize,
    pictures: &mut Pictures,
    images: &mut Assets<Image>,
) {
    let picture = item
        .picture
        .as_deref()
        .and_then(|path| pictures.get(path, images));
    row.spawn((
        item_parts(item, index),
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::axes(rem(1.0), rem(0.5)),
            border: UiRect::all(rem(0.15)),
            border_radius: BorderRadius::all(rem(0.4)),
            flex_grow: 1.0,
            flex_basis: px(0),
            row_gap: rem(0.3),
            ..default()
        },
    ))
    .with_children(|chip| {
        if let Some(picture) = picture {
            chip.spawn((
                ImageNode::new(picture),
                Node {
                    width: rem(8),
                    height: rem(6),
                    ..default()
                },
            ));
        }
        chip.spawn(text(item.label.clone(), 0.8, MUTED));
        chip.spawn(text(
            format!("<  {}  >", item.value.clone().unwrap_or_default()),
            1.1,
            TEXT,
        ));
        if let Some(note) = &item.note {
            chip.spawn(paragraph(note.clone(), 0.75, MUTED));
        }
    });
}

/// The parts every item shares: a button with a cursor stop, a click and a
/// moving mouse that move the cursor. Its box (`Node`) is the caller's.
fn item_parts(item: &Item, index: usize) -> impl Bundle {
    let id = item.id;
    (
        Button,
        ItemMarker { index },
        BackgroundColor(PANEL),
        BorderColor::all(Color::NONE),
        observe(move |_: On<Activate>, mut mouse: ResMut<MouseMenu>| {
            mouse.0.push(MouseAction::Click(id));
        }),
        observe(move |_: On<PointerMove>, mut mouse: ResMut<MouseMenu>| {
            mouse.0.push(MouseAction::Point(id));
        }),
    )
}

fn spawn_item(
    parent: &mut ChildSpawnerCommands<'_>,
    item: &Item,
    index: usize,
    pictures: &mut Pictures,
    images: &mut Assets<Image>,
) {
    let picture = item
        .picture
        .as_deref()
        .and_then(|path| pictures.get(path, images));
    match item.kind {
        ItemKind::Fly => {
            parent
                .spawn((
                    item_parts(item, index),
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::axes(rem(1.0), rem(1.6)),
                        border: UiRect::all(rem(0.2)),
                        border_radius: BorderRadius::all(rem(0.6)),
                        ..default()
                    },
                ))
                .with_child(text(item.label.clone(), 3.2, ACCENT));
        }
        ItemKind::Button => {
            parent
                .spawn((
                    item_parts(item, index),
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: rem(1.0),
                        padding: UiRect::axes(rem(1.0), rem(0.5)),
                        border: UiRect::all(rem(0.15)),
                        border_radius: BorderRadius::all(rem(0.4)),
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        ..default()
                    },
                ))
                .with_children(|button| {
                    if let Some(picture) = picture {
                        button.spawn((
                            ImageNode::new(picture),
                            Node {
                                width: rem(6.4),
                                height: rem(4.8),
                                flex_shrink: 0.0,
                                ..default()
                            },
                        ));
                    }
                    button
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: rem(0.2),
                            ..default()
                        })
                        .with_children(|words| {
                            let label = if item.current {
                                format!("{} (in use)", item.label)
                            } else {
                                item.label.clone()
                            };
                            words.spawn(text(label, 1.15, TEXT));
                            if let Some(note) = &item.note {
                                words.spawn(paragraph(note.clone(), 0.85, MUTED));
                            }
                        });
                });
        }
        ItemKind::Stop => {
            parent
                .spawn((
                    item_parts(item, index),
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::axes(rem(1.0), rem(0.5)),
                        border: UiRect::all(rem(0.15)),
                        border_radius: BorderRadius::all(rem(0.4)),
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        ..default()
                    },
                ))
                .with_children(|stop| {
                    stop.spawn(text(item.label.clone(), 1.15, TEXT));
                    stop.spawn(text(
                        format!("<  {}  >", item.value.clone().unwrap_or_default()),
                        1.15,
                        TEXT,
                    ));
                });
        }
    }
}
