use crate::{
    ActionToPerform, CameraCenter, Direction, GameState, Location, Map, Materials, Player, Speed,
    Tile, TILE_SIZE, TIME_STEP,
};
use bevy::prelude::*;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.add_startup_stage_after(
            "game_setup_map",
            "game_setup_actors",
            SystemStage::single(player_spawn.system()),
        )
        .add_system(player_input.system().label("input"))
        .add_system(player_actions.system().label("actions").after("input"));
    }
}

fn player_spawn(
    mut commands: Commands,
    materials: Res<Materials>,
    mut camera_center: ResMut<CameraCenter>,
    map_query: Query<(&Map)>,
) {
    // Create player sprite at map's spawn point (currently defaults to (1, 1))
    let mut spawn_point: Location = Location::default();
    if let Ok((current_map)) = map_query.single() {
        let map_spawn = &current_map.1;
        spawn_point.0 = map_spawn.0;
        spawn_point.1 = map_spawn.1;
        // println!(
        //     "Setting spawn point to {}, {}",
        //     spawn_point.0, spawn_point.1
        // );
    }
    // move camera to center on player
    camera_center.0 = spawn_point.0 as f32 * TILE_SIZE;
    camera_center.1 = spawn_point.1 as f32 * TILE_SIZE;

    commands
        .spawn_bundle(SpriteBundle {
            material: materials.player.clone(),
            sprite: Sprite::new(Vec2::new(TILE_SIZE * 2. / 3., TILE_SIZE * 2. / 3.)),
            transform: Transform {
                translation: Vec3::new(
                    spawn_point.0 as f32 * TILE_SIZE,
                    spawn_point.1 as f32 * TILE_SIZE,
                    10.,
                ),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Player)
        .insert(Speed::default())
        .insert(spawn_point);
}

fn player_input(
    mut commands: Commands,
    keyboard_input: Res<Input<KeyCode>>,
    mut game_state: ResMut<GameState>,
    map_query: Query<(&Map)>,
    mut player_query: Query<(&mut Location), With<Player>>,
) {
    // in the middle of a move, ignore inputs until finished
    if game_state.animating_actions {
        return;
    }

    if let Ok((mut location)) = player_query.single_mut() {
        if let Ok((current_map)) = map_query.single() {
            let map_data = &current_map.0;
            // allows 8 way movement
            let mut xdir: i32 = if keyboard_input.pressed(KeyCode::Left) {
                -1
            } else if keyboard_input.pressed(KeyCode::Right) {
                1
            } else {
                0
            };
            let mut ydir: i32 = if keyboard_input.pressed(KeyCode::Down) {
                -1
            } else if keyboard_input.pressed(KeyCode::Up) {
                1
            } else {
                0
            };
            let xnew = location.0 + xdir;
            let ynew = location.1 + ydir;
            // later track player's facing direction and set here
            // check for valid move
            if xnew < 0 || ynew < 0 {
                // moving out of bounds somehow?
                xdir = 0;
                ydir = 0;
            } else if let Some(tile) = map_data.get(ynew as usize, xnew as usize) {
                if tile == &Tile::Wall {
                    // moving into a wall tile
                    xdir = 0;
                    ydir = 0;
                } else if xdir != 0 && ydir != 0 {
                    // moving diagonally
                    if let (Some(xmove), Some(ymove)) = (
                        map_data.get(location.1 as usize, xnew as usize),
                        map_data.get(ynew as usize, location.0 as usize),
                    ) {
                        if xmove == &Tile::Wall || ymove == &Tile::Wall {
                            // trying to cut a corner!
                            xdir = 0;
                            ydir = 0;
                        }
                    }
                }
            } else {
                // moving out of bounds somehow?
                xdir = 0;
                ydir = 0;
            }

            // set animating_actions, mark location to move to, let other system handle animation
            // other system will also unset animating_actions
            if xdir != 0 || ydir != 0 {
                location.0 = xnew;
                location.1 = ynew;
                // println!("Intending to move to {}, {}", location.0, location.1);
                commands
                    .spawn()
                    .insert(ActionToPerform)
                    .insert(Direction(xdir, ydir));
                game_state.animating_actions = true;
            }
        }
    }
}

fn player_actions(
    mut commands: Commands,
    mut game_state: ResMut<GameState>,
    mut action_query: Query<(Entity, &Direction), With<ActionToPerform>>,
    mut camera_center: ResMut<CameraCenter>,
    mut player_query: Query<(&Speed, &mut Transform, &Location), With<Player>>,
) {
    if !game_state.animating_actions {
        return;
    }
    if let Ok((speed, mut player_tf, player_loc)) = player_query.single_mut() {
        if let Ok((move_entity, dir)) = action_query.single() {
            //get direction to move
            let move_x = dir.0 as f32;
            let move_y = dir.1 as f32;

            //get destination
            let dest_x = player_loc.0 as f32 * TILE_SIZE;
            let dest_y = player_loc.1 as f32 * TILE_SIZE;

            //prospective step
            let step_x = player_tf.translation.x + move_x * speed.0 * TILE_SIZE * TIME_STEP;
            let step_y = player_tf.translation.y + move_y * speed.0 * TILE_SIZE * TIME_STEP;

            //lock to next tile position if close enough and allow for input again
            let curr_dist_x = (dest_x - player_tf.translation.x).abs();
            let curr_dist_y = (dest_y - player_tf.translation.y).abs();
            let step_dist_x = (dest_x - step_x).abs();
            let step_dist_y = (dest_y - step_y).abs();

            if curr_dist_x <= step_dist_x && curr_dist_y <= step_dist_y {
                player_tf.translation.x = dest_x;
                player_tf.translation.y = dest_y;
                commands.entity(move_entity).despawn();
                game_state.animating_actions = false;
            } else {
                // otherwise, take the step
                player_tf.translation.x = step_x;
                player_tf.translation.y = step_y;
            }
            //keep the camera on the player
            camera_center.0 = player_tf.translation.x;
            camera_center.1 = player_tf.translation.y;
        }
    }
}
