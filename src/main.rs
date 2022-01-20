#![allow(unused)]
mod map;
mod player;

use array2d::Array2D;
use bevy::core::FixedTimestep;
use bevy::prelude::*;
use map::MapPlugin;
use player::PlayerPlugin;

const MAP_HEIGHT: usize = 32;
const MAP_WIDTH: usize = 56;
const WINDOW_HEIGHT: f32 = 600.;
const WINDOW_WIDTH: f32 = 800.;
const TILE_SIZE: f32 = 64.;
const TIME_STEP: f32 = 1. / 60.;

// region: Resources
pub struct Materials {
    player: Handle<ColorMaterial>,
    ground: Handle<ColorMaterial>,
    exit: Handle<ColorMaterial>,
    wall: Handle<ColorMaterial>,
    oob: Handle<ColorMaterial>,
}

#[derive(Clone, PartialEq)]
enum Tile {
    Ground,
    Wall,
}

#[derive(PartialEq)]
enum MapStyle {
    Standard,
    Circular,
    Cross,
}

struct WinSize {
    w: f32,
    h: f32,
}

#[derive(Default)]
struct CameraCenter(f32, f32);

#[derive(Default)]
struct GameState {
    has_map: bool,
    animating_actions: bool,
}
// endregion: Resources

// region: Components
struct Player;
struct Speed(f32); // speed is measured in tiles per second
impl Default for Speed {
    fn default() -> Self {
        Self(5.)
    }
}

struct ActionToPerform;
struct Direction(i32, i32);

struct IsCamera;

#[derive(Clone)]
struct Location(i32, i32);
impl Default for Location {
    fn default() -> Self {
        Self(1, 1)
    }
}

struct Map(Array2D<Tile>, Location);
struct MapElement;
// endregion: Components

fn main() {
    App::build()
        .insert_resource(ClearColor(Color::rgb(0.04, 0.04, 0.04)))
        .insert_resource(WindowDescriptor {
            title: "Rust Dungeon".to_string(),
            width: WINDOW_WIDTH,
            height: WINDOW_HEIGHT,
            ..Default::default()
        })
        .insert_resource(CameraCenter::default())
        .add_plugins(DefaultPlugins)
        .add_plugin(MapPlugin)
        .add_plugin(PlayerPlugin)
        .add_startup_system(setup.system())
        .add_system(update_camera.system().after("actions"))
        .add_system(update_map.system().after("actions"))
        .run();
}

fn setup(
    mut commands: Commands,
    // asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    // mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    mut windows: ResMut<Windows>,
) {
    let mut window = windows.get_primary_mut().unwrap();
    commands
        .spawn_bundle(OrthographicCameraBundle::new_2d())
        .insert(IsCamera);

    commands.insert_resource(Materials {
        player: materials.add(Color::rgb(0., 0.8, 0.).into()),
        ground: materials.add(Color::rgb(0.2, 0.2, 0.2).into()),
        exit: materials.add(Color::rgb(0.8, 0.8, 0.8).into()),
        wall: materials.add(Color::rgb(0.8, 0.2, 0.2).into()),
        oob: materials.add(Color::rgb(0.6, 0.2, 0.2).into()),
    });

    commands.insert_resource(WinSize {
        w: window.width(),
        h: window.height(),
    });
    // window.set_position(IVec2::new(1620, 100));
    commands.insert_resource(GameState::default());
    //create empty map
    // let mut new_map: Array2D<Tile> = Array2D::filled_with(Tile::Ground, MAP_HEIGHT, MAP_WIDTH);
    // //line edges of map with walls
    // for x in 0..MAP_WIDTH {
    //     new_map.set(0, x, Tile::Wall);
    //     new_map.set(MAP_HEIGHT - 1, x, Tile::Wall);
    // }

    // for y in 1..(MAP_HEIGHT - 1) {
    //     new_map.set(y, 0, Tile::Wall);
    //     new_map.set(y, MAP_WIDTH - 1, Tile::Wall);
    // }

    // commands.spawn().insert(Map(new_map));
}

fn update_camera(
    mut camera_query: Query<(&mut Transform), With<IsCamera>>,
    camera_center: Res<CameraCenter>,
) {
    if camera_center.is_changed() {
        if let Ok((mut camera_tf)) = camera_query.single_mut() {
            camera_tf.translation.x = camera_center.0;
            camera_tf.translation.y = camera_center.1;
        }
    }
}

fn update_map(
    mut commands: Commands,
    camera_center: Res<CameraCenter>,
    materials: Res<Materials>,
    game_state: ResMut<GameState>,
    map_query: Query<(&Map)>,
    tiles_query: Query<(Entity, &Location), With<MapElement>>,
) {
    if !game_state.has_map {
        return;
    }
    if camera_center.is_changed() {
        if let Ok((current_map)) = map_query.single() {
            // get range of tiles to draw
            let left_border = (camera_center.0 - WINDOW_WIDTH / 2.) / TILE_SIZE;
            let right_border = (camera_center.0 + WINDOW_WIDTH / 2.) / TILE_SIZE;
            let top_border = (camera_center.1 + WINDOW_HEIGHT / 2.) / TILE_SIZE;
            let bottom_border = (camera_center.1 - WINDOW_HEIGHT / 2.) / TILE_SIZE;
            let left_bound: i32 = left_border.floor() as i32;
            let right_bound: i32 = right_border.ceil() as i32;
            let top_bound: i32 = top_border.ceil() as i32;
            let bottom_bound: i32 = bottom_border.floor() as i32;

            let mut valid_tiles: Vec<&Location> = Vec::new();
            // clean up any tiles that are already drawn that are no longer in range
            for (tile_entity, loc) in tiles_query.iter() {
                if loc.0 > right_bound
                    || loc.0 < left_bound
                    || loc.1 > top_bound
                    || loc.1 < bottom_bound
                {
                    commands.entity(tile_entity).despawn();
                    // println!("Removing tile at {}, {}", loc.0, loc.1);
                } else {
                    valid_tiles.push(loc);
                }
            }
            // draw any tiles in the range that aren't already drawn
            for y in bottom_bound..=top_bound {
                for x in left_bound..=right_bound {
                    if !valid_tiles.iter().any(|e| e.0 == x && e.1 == y) {
                        let map_data = &current_map.0;
                        let possibly_tile = map_data.get(y as usize, x as usize);
                        let mat = match possibly_tile {
                            Some(tile) => match tile {
                                Tile::Ground => materials.ground.clone(),
                                Tile::Wall => materials.wall.clone(),
                            },
                            None => materials.oob.clone(),
                        };

                        // println!("Drawing tile at {}, {}", x, y);
                        commands
                            .spawn_bundle(SpriteBundle {
                                material: mat,
                                sprite: Sprite::new(Vec2::new(TILE_SIZE, TILE_SIZE)),
                                transform: Transform {
                                    translation: Vec3::new(
                                        x as f32 * TILE_SIZE,
                                        y as f32 * TILE_SIZE,
                                        5.,
                                    ),
                                    ..Default::default()
                                },
                                ..Default::default()
                            })
                            .insert(MapElement)
                            .insert(Location(x, y));
                    }
                }
            }
        }
    }
}
