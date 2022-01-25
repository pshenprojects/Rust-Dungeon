use crate::{
    FinishedMapEvent, GameState, Location, Map, MapElement, MapStyle, Materials, OnMap, Stairs,
    Tile, WinSize,
};
use array2d::Array2D;
use bevy::prelude::*;
use rand::{thread_rng, Rng};

pub struct MapPlugin;

#[derive(Clone, Default)]
struct Room {
    id: u32,
    dummy: bool,
    left: u32,
    width: u32,
    bottom: u32,
    height: u32,
}

struct MapMaker {
    columns: u32,
    rows: u32,
    rooms: u32,
    map_height: u32,
    map_width: u32,
    // style: MapStyle,
}

// REMINDER: Array2D get/set is rows then columns (y, x)
impl MapMaker {
    fn make(&mut self) -> (Map, Location) {
        let mut new_map: Array2D<Tile> = Array2D::filled_with(
            Tile::Wall,
            self.map_height as usize,
            self.map_width as usize,
        );
        let mut rng = thread_rng();
        let mut all_rooms: Vec<Room> = Vec::new();
        let mut connections: Vec<(u32, u32)> = Vec::new();
        let sector_width: u32 = self.map_width / self.columns;
        let sector_height: u32 = self.map_height / self.rows;
        let mut real_rooms: Vec<u32> = Vec::new();
        let mut can_merge_id: Vec<bool> = vec![true; (self.rows * self.columns) as usize];

        /* Default construction:
        pick r from range room_min..=room_max as # of rooms
        choose r IDs from 0..(self.rows*self.columns)
        iterate through sector columns + rows
        if column + self.columns * row == id, make a real room with random dimensions
        at least 5 x 4, up to sector_width - 1 and sector_height - 1
        else make a dummy 1x1 room
        add this Room to all_rooms
        next: iterate through all_rooms
        real rooms need at least 1 connection made to an adjacent room, up to 4 connections
        dummy rooms can be ignored for now.
        check for strongly connected layout:
        all real rooms are accessible
        dummy rooms are fine being inaccessible
        after strongly connected is proven, delete dummy rooms that aren't connected at all
        after rooms and connections are defined, call make_room for every room
        and make_corridor for every connection
        finally, pick a room to spawn in and label its spawn point
        */

        // pick sectors to hold real rooms
        let mut sector_ids: Vec<u32> = (0..(self.rows * self.columns)).collect();
        if self.rooms >= self.rows * self.columns {
            real_rooms = sector_ids;
        } else {
            for i in 0..self.rooms {
                let pick = rng.gen_range(0..sector_ids.len());
                real_rooms.push(sector_ids.swap_remove(pick));
            }
        }

        real_rooms.sort();

        // create a room in every sector
        for y in 0..self.rows {
            for x in 0..self.columns {
                let curr_id = x + self.columns * y;
                if real_rooms.iter().any(|&id| id == curr_id) {
                    let room_width = rng.gen_range(5..sector_width - 2);
                    let room_height = rng.gen_range(4..sector_height - 2);
                    let room_left = rng.gen_range(2..sector_width - room_width);
                    let room_bottom = rng.gen_range(2..sector_height - room_height);
                    all_rooms.push(Room {
                        id: curr_id,
                        dummy: false,
                        left: room_left + x * sector_width,
                        width: room_width,
                        bottom: room_bottom + y * sector_height,
                        height: room_height,
                    });
                } else {
                    let room_left = rng.gen_range(2..sector_width - 1);
                    let room_bottom = rng.gen_range(2..sector_height - 1);
                    all_rooms.push(Room {
                        id: curr_id,
                        dummy: true,
                        left: room_left + x * sector_width,
                        width: 1,
                        bottom: room_bottom + y * sector_height,
                        height: 1,
                    });
                    can_merge_id[curr_id as usize] = false;
                }
            }
        }
        // pick a random spawn location within a random real room
        let pick_spawn = rng.gen_range(0..real_rooms.len());
        let spawn_room_id = real_rooms[pick_spawn];

        // pick a random exit location within a random real room
        let pick_exit = rng.gen_range(0..real_rooms.len());
        let exit_room_id = real_rooms[pick_exit];

        /* generate corridors:
        for every room, consider all possible connections to adjacent rooms
        pick 1-4 of them for real rooms, dummy rooms can be skipped
        then, pass list of connections to cluster testing function.
        if it fails, keep the list but add more connections and try again until it succeeds
        */
        for room in all_rooms.iter() {
            let mut sectors_adj: Vec<u32> = Vec::new();
            if room.id % self.columns == 0 {
                sectors_adj.push(room.id + 1);
            } else if (room.id + 1) % self.columns == 0 {
                sectors_adj.push(room.id - 1);
            } else {
                sectors_adj.push(room.id - 1);
                sectors_adj.push(room.id + 1);
            }
            if room.id < self.columns {
                sectors_adj.push(room.id + self.columns);
            } else if room.id >= self.columns * (self.rows - 1) {
                sectors_adj.push(room.id - self.columns);
            } else {
                sectors_adj.push(room.id - self.columns);
                sectors_adj.push(room.id + self.columns);
            }
            // rng chance to skip making connections to a dummy room
            if room.dummy && rng.gen_bool(0.5) {
                continue;
            } else {
                let nconnections = rng.gen_range(1..=sectors_adj.len());
                for i in 0..nconnections {
                    let pick = rng.gen_range(0..sectors_adj.len());
                    let id = sectors_adj.swap_remove(pick);
                    if !already_has_connection(&connections, room.id, id) {
                        // when adding a new connection, always try to keep it smaller-to-larger
                        if room.id > id {
                            connections.push((id, room.id));
                        } else {
                            connections.push((room.id, id));
                        }
                    }
                }
            }
        }
        /* check for fully connected: perform initial check
        if initial check is failed, pick a room that is adjacent to the cluster,
        generate a connection (that doesn't already exist) off of it, then try again
        once cluster contains all rooms, clean up any dummy rooms that have no connections
        */
        let mut cluster = get_cluster(&connections, spawn_room_id);
        while !has_all(&cluster, &real_rooms) {
            let mut potential_connection: Vec<(u32, u32)> = Vec::new();
            for &id in cluster.iter() {
                if (id + 1) % self.columns != 0 {
                    let right = id + 1;
                    if !already_has_connection(&connections, id, right) {
                        if cluster.iter().all(|&id| id != right) {
                            potential_connection.push((id, right));
                        }
                    }
                }
                if id % self.columns != 0 {
                    let left = id - 1;
                    if !already_has_connection(&connections, id, left) {
                        if cluster.iter().all(|&id| id != left) {
                            potential_connection.push((id, left));
                        }
                    }
                }
                if id < self.columns * (self.rows - 1) {
                    let up = id + self.columns;
                    if !already_has_connection(&connections, id, up) {
                        if cluster.iter().all(|&id| id != up) {
                            potential_connection.push((id, up));
                        }
                    }
                }
                if id >= self.columns {
                    let down = id - self.columns;
                    if !already_has_connection(&connections, id, down) {
                        if cluster.iter().all(|&id| id != down) {
                            potential_connection.push((id, down));
                        }
                    }
                }
            }
            let pick = rng.gen_range(0..potential_connection.len());
            let (id1, id2) = potential_connection[pick];
            // when adding a new connection, always try to keep it smaller-to-larger
            if id1 > id2 {
                connections.push((id2, id1));
            } else {
                connections.push((id1, id2));
            }
            // println!(
            //     "adding connection between sectors {} and {}",
            //     potential_connection[pick].0, potential_connection[pick].1,
            // );
            cluster = get_cluster(&connections, spawn_room_id);
        }

        // now, draw all the rooms that are in the complete cluster
        for room in all_rooms.iter() {
            if cluster.iter().any(|&id| id == room.id) {
                make_room(&mut new_map, &room);
                // } else {
                //     println!(
                //         "Skipping room {} because it's not connected to anything",
                //         room.id
                //     );
            }
        }
        // now, draw all the connections: id1 should always be smaller than id2
        for connect in connections.iter() {
            let &(id1, id2) = connect;
            // skip any connections that do not involve the complete cluster
            if !cluster.iter().any(|&id| id == id1 || id == id2) {
                continue;
            }
            let diff = id2 - id1;
            if let Some(room1) = all_rooms.iter().find(|&r| r.id == id1) {
                if let Some(room2) = all_rooms.iter().find(|&r| r.id == id2) {
                    // println!("Connecting sectors {} and {}", id1, id2);
                    // if both sides of the connections are real rooms
                    // 10% chance of merging if they aren't already merged elsewhere
                    if can_merge_id[id1 as usize] && can_merge_id[id2 as usize] && rng.gen_bool(0.1)
                    {
                        merge_rooms(&mut new_map, &room1, &room2);
                        can_merge_id[id1 as usize] = false;
                        can_merge_id[id2 as usize] = false;
                    }
                    // if horizontal
                    else if diff <= 1 {
                        let xleft: i32 = (room1.left + room1.width - 1) as i32;
                        let random_yleft: i32 =
                            (room1.bottom + rng.gen_range(0..room1.height)) as i32;
                        let xright: i32 = room2.left as i32;
                        let random_yright: i32 =
                            (room2.bottom + rng.gen_range(0..room2.height)) as i32;
                        let point1: Location = Location(xleft, random_yleft);
                        let point2: Location = Location(xright, random_yright);
                        // println!(
                        //     "Drawing horizontal connection between {}, {} and {}, {}",
                        //     point1.0, point1.1, point2.0, point2.1
                        // );
                        let random_mid: i32 = rng.gen_range(xleft + 2..xright - 1);
                        make_corridor_horizontal(&mut new_map, &point1, &point2, random_mid);
                    } else {
                        let ybottom: i32 = (room1.bottom + room1.height - 1) as i32;
                        let random_xbottom: i32 =
                            (room1.left + rng.gen_range(0..room1.width)) as i32;
                        let ytop: i32 = room2.bottom as i32;
                        let random_xtop: i32 = (room2.left + rng.gen_range(0..room2.width)) as i32;
                        let point1: Location = Location(random_xbottom, ybottom);
                        let point2: Location = Location(random_xtop, ytop);
                        // println!(
                        //     "Drawing vertical connection between {}, {} and {}, {}",
                        //     point1.0, point1.1, point2.0, point2.1
                        // );
                        let random_mid: i32 = rng.gen_range(ybottom + 2..ytop - 1);
                        make_corridor_vertical(&mut new_map, &point1, &point2, random_mid);
                    }
                }
            }
        }
        // println!(
        //     "Picked index {} of {} with room id {}",
        //     pick_spawn,
        //     real_rooms.len() - 1,
        //     spawn_room_id
        // );
        if let Some(spawn_room) = all_rooms.iter().find(|&r| r.id == spawn_room_id) {
            let random_spawn_x = spawn_room.left + rng.gen_range(0..spawn_room.width);
            let random_spawn_y = spawn_room.bottom + rng.gen_range(0..spawn_room.height);
            if let Some(exit_room) = all_rooms.iter().find(|&r| r.id == exit_room_id) {
                let random_exit_x = exit_room.left + rng.gen_range(0..exit_room.width);
                let random_exit_y = exit_room.bottom + rng.gen_range(0..exit_room.height);
                // println!("Setting exit point to {}, {}", random_exit_x, random_exit_y);
                (
                    Map(
                        new_map,
                        Location(random_spawn_x as i32, random_spawn_y as i32),
                    ),
                    Location(random_exit_x as i32, random_exit_y as i32),
                )
            } else {
                (
                    Map(
                        new_map,
                        Location(random_spawn_x as i32, random_spawn_y as i32),
                    ),
                    Location(random_spawn_x as i32, random_spawn_y as i32),
                )
            }
        } else {
            (Map(new_map, Location::default()), Location::default())
        }
    }
}

fn already_has_connection(conn_list: &Vec<(u32, u32)>, id1: u32, id2: u32) -> bool {
    conn_list
        .iter()
        .any(|&(e1, e2)| (e1 == id1 && e2 == id2) || (e1 == id2 && e2 == id1))
}

fn get_cluster(conn_list: &Vec<(u32, u32)>, start: u32) -> Vec<u32> {
    let mut cluster: Vec<u32> = vec![start];
    let mut cluster_size: usize = 0;
    let mut max_id: u32 = start;
    while cluster.len() != cluster_size {
        cluster_size = cluster.len();
        for &(id1, id2) in conn_list.iter() {
            let has_id1 = cluster.contains(&id1);
            let has_id2 = cluster.contains(&id2);
            match (has_id1, has_id2) {
                (false, true) => cluster.push(id1),
                (true, false) => cluster.push(id2),
                (_, _) => continue,
            }
        }
    }
    cluster.sort();
    cluster
}

fn has_all(cluster: &Vec<u32>, rooms: &Vec<u32>) -> bool {
    // println!("Testing cluster:");
    // for i in cluster.iter() {
    //     print!("{}, ", i);
    // }
    // println!();
    // println!("With rooms:");
    // for i in rooms.iter() {
    //     print!("{}, ", i);
    // }
    // println!();
    let mut cluster_iter = cluster.iter();
    rooms.iter().all(|&id| cluster_iter.any(|&rid| rid == id))
}

fn make_room(map: &mut Array2D<Tile>, room: &Room) {
    for y in 0..room.height {
        for x in 0..room.width {
            let real_x: usize = (x + room.left) as usize;
            let real_y: usize = (y + room.bottom) as usize;
            map.set(real_y, real_x, Tile::Ground);
        }
    }
    // println!(
    //     "Creating a {}x{} room at {}, {} with id {}",
    //     room.width, room.height, room.left, room.bottom, room.id
    // );
}

fn merge_rooms(map: &mut Array2D<Tile>, room1: &Room, room2: &Room) {
    let big_left = room1.left.min(room2.left);
    let big_bottom = room1.bottom.min(room2.bottom);
    let big_right = (room1.left + room1.width).max(room2.left + room2.width);
    let big_top = (room1.bottom + room1.height).max(room2.bottom + room2.height);
    for y in big_bottom..big_top {
        for x in big_left..big_right {
            map.set(y as usize, x as usize, Tile::Ground);
        }
    }
}

// make sure to pass point arguments left to right, and bridge_x is between the two points
fn make_corridor_horizontal(
    map: &mut Array2D<Tile>,
    point1: &Location,
    point2: &Location,
    bridge_x: i32,
) {
    for x in point1.0..=bridge_x {
        map.set(point1.1 as usize, x as usize, Tile::Ground);
    }
    for x in bridge_x..=point2.0 {
        map.set(point2.1 as usize, x as usize, Tile::Ground);
    }
    if point1.1 < point2.1 {
        for y in point1.1..=point2.1 {
            map.set(y as usize, bridge_x as usize, Tile::Ground);
        }
    } else if point1.1 > point2.1 {
        for y in point2.1..=point1.1 {
            map.set(y as usize, bridge_x as usize, Tile::Ground);
        }
    }
}

// make sure to pass point arguments bottom to top, and bridge_y is between the two points
fn make_corridor_vertical(
    map: &mut Array2D<Tile>,
    point1: &Location,
    point2: &Location,
    bridge_y: i32,
) {
    for y in point1.1..=bridge_y {
        map.set(y as usize, point1.0 as usize, Tile::Ground);
    }
    for y in bridge_y..=point2.1 {
        map.set(y as usize, point2.0 as usize, Tile::Ground);
    }
    if point1.0 < point2.0 {
        for x in point1.0..=point2.0 {
            map.set(bridge_y as usize, x as usize, Tile::Ground);
        }
    } else if point1.0 > point2.0 {
        for x in point2.0..=point1.0 {
            map.set(bridge_y as usize, x as usize, Tile::Ground);
        }
    }
}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.insert_resource(MapMaker {
            columns: 3,
            rows: 2,
            rooms: 2,
            map_height: 32,
            map_width: 56,
        })
        .add_startup_stage("game_setup_map", SystemStage::single(create_map.system()))
        .add_event::<FinishedMapEvent>()
        .add_system(cleanup_map.system().label("cleanup").after("actions"))
        .add_system(create_map.system().after("cleanup"));
    }
}

fn create_map(
    mut commands: Commands,
    mut map_maker: ResMut<MapMaker>,
    mut game_state: ResMut<GameState>,
    materials: Res<Materials>,
    window: Res<WinSize>,
) {
    if !game_state.has_map {
        let mut rng = thread_rng();
        let c: u32 = rng.gen_range(3..=4);
        let r: u32 = rng.gen_range(2..=4);
        map_maker.columns = c;
        map_maker.rows = r;
        map_maker.rooms = rng.gen_range(2..=c * r);
        let (map, exit) = map_maker.make();
        commands.spawn().insert(map);
        commands
            .spawn_bundle(SpriteBundle {
                material: materials.exit.clone(),
                sprite: Sprite::new(Vec2::new(window.tile * 7. / 8., window.tile * 7. / 8.)),
                transform: Transform {
                    translation: Vec3::new(
                        exit.0 as f32 * window.tile,
                        exit.1 as f32 * window.tile,
                        6.,
                    ),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(Stairs)
            .insert(OnMap(exit));
        game_state.has_map = true;
    }
}

fn cleanup_map(
    mut commands: Commands,
    mut ev_finished_map: EventReader<FinishedMapEvent>,
    mut game_state: ResMut<GameState>,
    map_query: Query<Entity, With<Map>>,
    object_query: Query<Entity, With<OnMap>>,
    tiles_query: Query<Entity, With<MapElement>>,
) {
    for ev in ev_finished_map.iter() {
        game_state.has_map = false;
        for obj_entity in object_query.iter() {
            commands.entity(obj_entity).despawn();
        }
        for tiles_entity in tiles_query.iter() {
            commands.entity(tiles_entity).despawn();
        }
        for map_entity in map_query.iter() {
            commands.entity(map_entity).despawn();
        }
    }
}
