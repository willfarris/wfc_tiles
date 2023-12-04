use std::{collections::HashMap, io::Read, path::Path, time::Duration};

use json::JsonValue;

use rand::{seq::SliceRandom, thread_rng};

const GRID_SIZE: isize = 50;

#[derive(Debug)]
struct BoardCharacter {
    pub character: String,
    pub valid_neighbors: HashMap<String, Vec<String>>,
}

type AdjacencyMap = HashMap<String, BoardCharacter>;

trait WFCAdjacencyMap {
    fn create(prototypes_path: &Path) -> Self;
}

impl WFCAdjacencyMap for AdjacencyMap {
    fn create(prototypes_path: &Path) -> Self {
        let mut buffer = String::new();
        std::fs::File::open(prototypes_path)
            .unwrap()
            .read_to_string(&mut buffer)
            .unwrap();
        let mut prototypes_json = json::parse(buffer.as_str()).unwrap();

        let mut prototype_map: AdjacencyMap = HashMap::new();

        for (tile_name, tile_description) in prototypes_json.entries_mut() {
            let character_json: JsonValue = tile_description.remove("char");
            let character = character_json.as_str().unwrap().to_string();

            let valid_neighbors_json: JsonValue = tile_description.remove("valid_neighbors");
            let mut valid_neighbors = HashMap::new(); // [Vec<String>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
            for (direction, valid_neighbor_list) in valid_neighbors_json.entries() {
                valid_neighbors.insert(direction.to_string(), Vec::new());
                if let Some(list) = valid_neighbors.get_mut(&direction.to_string()) {
                    for valid_neighbor_json in valid_neighbor_list.members() {
                        list.push(valid_neighbor_json.as_str().unwrap().to_string());
                    }
                }
            }

            let board_character = BoardCharacter {
                character,
                valid_neighbors,
            };
            println!("{} - {:?}", tile_name, board_character.valid_neighbors);
            prototype_map.insert(tile_name.to_string(), board_character);
        }
        prototype_map
    }
}

type Vec2 = [isize; 2];
type Domain = Vec<String>;

enum Tile {
    Collapsed(String),
    Uncollapsed(Domain),
}

impl Tile {
    fn default_domain(prototype_map: &AdjacencyMap) -> Self {
        let mut default_domain = Vec::new();
        for (tile_name, _description) in prototype_map.iter() {
            default_domain.push(tile_name.clone());
        }
        Tile::Uncollapsed(default_domain)
    }

    fn domain_from(val: String) -> Self {
        let domain = [val].into();
        Tile::Uncollapsed(domain)
    }
}

type Board = HashMap<Vec2, Tile>;

trait WFCBoard {
    fn create(prototype_map: &AdjacencyMap, grid_size: isize) -> Self;
    fn get_lowest_entropy(&self) -> Vec2;
    fn is_collapsed(&self) -> bool;
    fn propagate_collapse(
        &mut self,
        prototype_map: &AdjacencyMap,
        pos: &Vec2,
    ) -> Vec<(Vec2, String)>;
    fn restore_domains(&mut self, tiles: Vec<(Vec2, String)>);
    fn is_valid_placement(&self, prototype_map: &AdjacencyMap, val: &String, pos: &Vec2) -> bool;
    fn collapse(&mut self, prototype_map: &AdjacencyMap) -> bool;
    fn print(&self, prototype_map: &AdjacencyMap, grid_size: isize);
}

impl WFCBoard for Board {
    fn create(prototype_map: &AdjacencyMap, grid_size: isize) -> Self {
        let mut board = HashMap::new();
        for row in 0..grid_size {
            for col in 0..grid_size {
                board.insert([row, col], Tile::default_domain(prototype_map));
            }
        }

        board
    }

    fn get_lowest_entropy(&self) -> Vec2 {
        let mut lowest_len = usize::MAX;
        let mut lowest_index = [0, 0];

        for (pos, tile) in self.iter() {
            match tile {
                Tile::Collapsed(_) => continue,
                Tile::Uncollapsed(domain) => {
                    if domain.len() <= lowest_len {
                        lowest_len = domain.len();
                        lowest_index = pos.clone();
                    }
                }
            }
        }

        lowest_index
    }

    fn is_collapsed(&self) -> bool {
        for (_pos, tile) in self.iter() {
            if let Tile::Uncollapsed(_) = tile {
                return false;
            }
        }

        true
    }

    fn propagate_collapse(
        &mut self,
        prototype_map: &AdjacencyMap,
        pos: &Vec2,
    ) -> Vec<(Vec2, String)> {
        let mut modified = Vec::new();

        let val = match self.get(pos).unwrap() {
            Tile::Uncollapsed(_) => return modified,
            Tile::Collapsed(val) => val.clone(),
        };
        let pr = pos[0];
        let pc = pos[1];

        let adjacent = [
            ("right", [pr - 1, pc]),
            ("left", [pr + 1, pc]),
            ("above", [pr, pc - 1]),
            ("below", [pr, pc + 1]),
        ];

        for (adjacent_name, adjacent_pos) in adjacent {
            if let Some(tile) = self.get_mut(&adjacent_pos) {
                if let Tile::Uncollapsed(domain) = tile {
                    domain.retain(|adjacent_domain_element| {
                        // retain adjacent_domain_element if prototype_map[adjacent_domain_element].valid_neighbors["right"].containts(val)
                        if !prototype_map[adjacent_domain_element].valid_neighbors[adjacent_name]
                            .contains(&val)
                        {
                            modified.push((adjacent_pos, adjacent_domain_element.clone()));
                            return false;
                        }
                        true
                    })
                }
            }
        }

        modified
    }

    fn restore_domains(&mut self, tiles: Vec<(Vec2, String)>) {
        for (pos, tile_name) in tiles {
            if let Some(tile) = self.get_mut(&pos) {
                if let Tile::Uncollapsed(domain) = tile {
                    domain.push(tile_name)
                } else {
                    self.remove(&pos);
                    self.insert(pos, Tile::domain_from(tile_name));
                }
            }
        }
    }

    fn is_valid_placement(&self, prototype_map: &AdjacencyMap, val: &String, pos: &Vec2) -> bool {
        let pr = pos[0];
        let pc = pos[1];

        let adjacent = [
            ("right", [pr - 1, pc]),
            ("left", [pr + 1, pc]),
            ("above", [pr, pc - 1]),
            ("below", [pr, pc + 1]),
        ];

        for (adjacent_name, adjacent_pos) in adjacent {
            if let Some(tile) = self.get(&adjacent_pos) {
                if let Tile::Collapsed(adjacent_val) = tile {
                    if !prototype_map.get(adjacent_val).unwrap().valid_neighbors[adjacent_name]
                        .contains(val)
                    {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn collapse(&mut self, prototype_map: &AdjacencyMap) -> bool {
        self.print(prototype_map, GRID_SIZE);
        if self.is_collapsed() {
            return true;
        }

        let pos = self.get_lowest_entropy();
        let mut possible_tiles = match self.get(&pos).unwrap() {
            Tile::Collapsed(_) => panic!("lowest entropy shouldn't be collapsed"),
            Tile::Uncollapsed(domain) => domain.clone(),
        };

        possible_tiles.shuffle(&mut thread_rng());

        for possible_tile in possible_tiles.iter() {
            if self.is_valid_placement(prototype_map, possible_tile, &pos) {
                let saved_domain = if let Tile::Uncollapsed(domain) = self.get(&pos).unwrap() {
                    domain.clone()
                } else {
                    panic!("lowest entropy tile shouldn't be collapsed")
                };

                self.remove(&pos);
                self.insert(pos, Tile::Collapsed(possible_tile.clone()));
                let modified = self.propagate_collapse(prototype_map, &pos);
                if self.collapse(prototype_map) {
                    return true;
                }
                self.remove(&pos);
                self.insert(pos, Tile::Uncollapsed(saved_domain));
                self.restore_domains(modified);
            }
        }

        false
    }

    fn print(&self, prototype_map: &AdjacencyMap, grid_size: isize) {
        std::thread::sleep(Duration::from_millis(10));
        print!("\x1B[2J\x1B[1;1H");
        for c in 0..grid_size {
            for r in 0..grid_size {
                let pos = [r, grid_size - c];
                if let Some(tile) = self.get(&pos) {
                    match tile {
                        Tile::Collapsed(tile_name) => {
                            print!("{}", prototype_map[tile_name].character);
                        }
                        Tile::Uncollapsed(_domain) => print!("."),
                    }
                }
            }
            println!();
        }
    }
}

fn main() {
    let prototype_map = AdjacencyMap::create(Path::new("prototypes.json"));
    let mut board = Board::create(&prototype_map, GRID_SIZE);

    board.collapse(&prototype_map);

    board.print(&prototype_map, GRID_SIZE);
}
