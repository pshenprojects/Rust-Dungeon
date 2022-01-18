use crate::{GameState, Location, Map, MapStyle, Tile, MAP_HEIGHT, MAP_WIDTH};
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
    col_min: u32,
    col_max: u32,
    row_min: u32,
    row_max: u32,
    room_min: u32,
    room_max: u32,
    // style: MapStyle,
}

// REMINDER: Array2D get/set is rows then columns (y, x)
impl MapMaker {
    fn make(&mut self) -> Map {
        let mut new_map: Array2D<Tile> = Array2D::filled_with(Tile::Wall, MAP_HEIGHT, MAP_WIDTH);
        let mut rng = thread_rng();
        let mut all_rooms: Vec<Room> = Vec::new();
        let mut connections: Vec<(u32, u32)> = Vec::new();
        let sector_rows: u32 = rng.gen_range(self.row_min..=self.row_max);
        let sector_columns: u32 = rng.gen_range(self.col_min..=self.col_max);
        let sector_width: u32 = MAP_WIDTH as u32 / sector_columns;
        let sector_height: u32 = MAP_HEIGHT as u32 / sector_rows;
        let mut real_rooms: Vec<u32> = Vec::new();

        /* Default construction:
        pick r from range room_min..=room_max as # of rooms
        choose r IDs from 0..(sector_rows*sector_columns)
        iterate through sector columns + rows
        if column + sector_columns * row == id, make a real room with random dimensions
        at least 5 x 4, up to sector_width - 1 and sector_height - 1
        else make a dummy 1x1 room
        add this Room to all_rooms
        next: iterate through all_rooms
        real rooms need at least 1 connection made to an adjacent room, up to 4 connections
        dummy rooms need 0 connections, or 2-4 (no dead ends? are dead ends fine?)
        check for strongly connected layout:
        all real rooms are accessible (tree search thru connections)
        dummy rooms are fine being inaccessible
        if not strongly connected... rebuild connections from start?
        after strongly connected is proven, delete dummy rooms that aren't connected at all
        after rooms and connections are defined, call make_room for every room
        and make_corridor for every connection
        finally, pick a room to spawn in and label its spawn point
        */
        let nrooms: u32 = rng.gen_range(self.room_min..=self.room_max);
        let mut sector_ids: Vec<u32> = (0..(sector_rows * sector_columns)).collect();
        if nrooms >= sector_rows * sector_columns {
            real_rooms = sector_ids;
        } else {
            for i in 0..nrooms {
                let pick = rng.gen_range(0..sector_ids.len());
                real_rooms.push(sector_ids.swap_remove(pick));
            }
        }

        real_rooms.sort();

        for y in 0..sector_rows {
            for x in 0..sector_columns {
                let curr_id = x + sector_columns * y;
                if real_rooms.iter().any(|&id| id == curr_id) {
                    let room_width = rng.gen_range(5..sector_width - 1);
                    let room_height = rng.gen_range(4..sector_height - 1);
                    let room_left = rng.gen_range(1..(sector_width - room_width));
                    let room_bottom = rng.gen_range(1..(sector_height - room_height));
                    all_rooms.push(Room {
                        id: curr_id,
                        dummy: false,
                        left: room_left + x * sector_width,
                        width: room_width,
                        bottom: room_bottom + y * sector_height,
                        height: room_height,
                    })
                } else {
                    let room_left = rng.gen_range(1..sector_width - 1);
                    let room_bottom = rng.gen_range(1..sector_height - 1);
                    all_rooms.push(Room {
                        id: curr_id,
                        dummy: true,
                        left: room_left + x * sector_width,
                        width: 1,
                        bottom: room_bottom + y * sector_height,
                        height: 1,
                    })
                }
            }
        }
        // pick a random spawn location within a random real room
        let pick_spawn = rng.gen_range(0..real_rooms.len());
        let spawn_room_id = real_rooms[pick_spawn];

        /* generate corridors:
        for every room, consider all possible connections to adjacent rooms
        pick 1-4 of them for real rooms, 0 or 2-4 of them for dummy rooms
        then, pass list of connections to cluster testing function.
        if it fails, keep the list but add more connections and try again until it succeeds
        */
        for room in all_rooms.iter() {
            let mut sectors_adj: Vec<u32> = Vec::new();
            if room.id % sector_columns == 0 {
                sectors_adj.push(room.id + 1);
            } else if (room.id + 1) % sector_columns == 0 {
                sectors_adj.push(room.id - 1);
            } else {
                sectors_adj.push(room.id - 1);
                sectors_adj.push(room.id + 1);
            }
            if room.id < sector_columns {
                sectors_adj.push(room.id + sector_columns);
            } else if room.id >= sector_columns * (sector_rows - 1) {
                sectors_adj.push(room.id - sector_columns);
            } else {
                sectors_adj.push(room.id - sector_columns);
                sectors_adj.push(room.id + sector_columns);
            }
            // dummy rooms have a 50% chance of being skipped over for connection making
            // test: skipping dummy rooms entirely
            if room.dummy {
                // && rng.gen_bool(0.5) {
                continue;
            } else {
                let nconnections = rng.gen_range(1..sectors_adj.len());
                for i in 0..nconnections {
                    let pick = rng.gen_range(0..sectors_adj.len());
                    let id = sectors_adj.swap_remove(pick);
                    if !already_has_connection(&connections, room.id, id) {
                        connections.push((room.id, id));
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
                if (id + 1) % sector_columns != 0 {
                    let right = id + 1;
                    if !already_has_connection(&connections, id, right) {
                        if cluster.iter().all(|&id| id != right) {
                            potential_connection.push((id, right));
                        }
                    }
                }
                if id % sector_columns != 0 {
                    let left = id - 1;
                    if !already_has_connection(&connections, id, left) {
                        if cluster.iter().all(|&id| id != left) {
                            potential_connection.push((id, left));
                        }
                    }
                }
                if id < sector_columns * (sector_rows - 1) {
                    let up = id + sector_columns;
                    if !already_has_connection(&connections, id, up) {
                        if cluster.iter().all(|&id| id != up) {
                            potential_connection.push((id, up));
                        }
                    }
                }
                if id >= sector_columns {
                    let down = id - sector_columns;
                    if !already_has_connection(&connections, id, down) {
                        if cluster.iter().all(|&id| id != down) {
                            potential_connection.push((id, down));
                        }
                    }
                }
            }
            let pick = rng.gen_range(0..potential_connection.len());
            connections.push(potential_connection[pick]);
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
        // now, draw all the connections
        for connect in connections.iter() {
            let &(id1, id2) = connect;
            let diff = (id1 as i32 - id2 as i32).abs();
            if let Some(room1) = all_rooms.iter().find(|&r| r.id == id1) {
                if let Some(room2) = all_rooms.iter().find(|&r| r.id == id2) {
                    // println!("Connecting sectors {} and {}", id1, id2);
                    // if horizontal
                    if diff <= 1 {
                        if id1 < id2 {
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
                            let random_mid: i32 = rng.gen_range(xleft + 1..xright);
                            make_corridor_horizontal(&mut new_map, &point1, &point2, random_mid);
                        } else {
                            let xleft: i32 = (room2.left + room2.width - 1) as i32;
                            let random_yleft: i32 =
                                (room2.bottom + rng.gen_range(0..room2.height)) as i32;
                            let xright: i32 = room1.left as i32;
                            let random_yright: i32 =
                                (room1.bottom + rng.gen_range(0..room1.height)) as i32;
                            let point1: Location = Location(xleft, random_yleft);
                            let point2: Location = Location(xright, random_yright);
                            // println!(
                            //     "Drawing horizontal connection between {}, {} and {}, {}",
                            //     point1.0, point1.1, point2.0, point2.1
                            // );
                            let random_mid: i32 = rng.gen_range(xleft + 1..xright);
                            make_corridor_horizontal(&mut new_map, &point1, &point2, random_mid);
                        }
                    } else {
                        if id1 < id2 {
                            let ybottom: i32 = (room1.bottom + room1.height - 1) as i32;
                            let random_xbottom: i32 =
                                (room1.left + rng.gen_range(0..room1.width)) as i32;
                            let ytop: i32 = room2.bottom as i32;
                            let random_xtop: i32 =
                                (room2.left + rng.gen_range(0..room2.width)) as i32;
                            let point1: Location = Location(random_xbottom, ybottom);
                            let point2: Location = Location(random_xtop, ytop);
                            // println!(
                            //     "Drawing vertical connection between {}, {} and {}, {}",
                            //     point1.0, point1.1, point2.0, point2.1
                            // );
                            let random_mid: i32 = rng.gen_range(ybottom + 1..ytop);
                            make_corridor_vertical(&mut new_map, &point1, &point2, random_mid);
                        } else {
                            let ybottom: i32 = (room2.bottom + room2.height - 1) as i32;
                            let random_xbottom: i32 =
                                (room2.left + rng.gen_range(0..room2.width)) as i32;
                            let ytop: i32 = room1.bottom as i32;
                            let random_xtop: i32 =
                                (room1.left + rng.gen_range(0..room1.width)) as i32;
                            let point1: Location = Location(random_xbottom, ybottom);
                            let point2: Location = Location(random_xtop, ytop);
                            // println!(
                            //     "Drawing vertical connection between {}, {} and {}, {}",
                            //     point1.0, point1.1, point2.0, point2.1
                            // );
                            let random_mid: i32 = rng.gen_range(ybottom + 1..ytop);
                            make_corridor_vertical(&mut new_map, &point1, &point2, random_mid);
                        }
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
            let random_x = spawn_room.left + rng.gen_range(0..spawn_room.width);
            let random_y = spawn_room.bottom + rng.gen_range(0..spawn_room.height);
            Map(new_map, Location(random_x as i32, random_y as i32))
        } else {
            Map(new_map, Location::default())
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
            col_min: 3,
            col_max: 4,
            row_min: 2,
            row_max: 4,
            room_min: 2,
            room_max: 10,
        })
        .add_startup_stage("game_setup_map", SystemStage::single(create_map.system()));
    }
}

fn create_map(
    mut commands: Commands,
    mut map_maker: ResMut<MapMaker>,
    mut game_state: ResMut<GameState>,
) {
    let map = map_maker.make();
    commands.spawn().insert(map);
    game_state.has_map = true;
}
