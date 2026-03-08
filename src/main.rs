use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use rand::Rng;
use std::time::Instant;

const BOARD_W: usize = 10;
const BOARD_H: usize = 20;
const CELL: usize = 24;
const BOARD_X: usize = 360;
const BOARD_Y: usize = 60;
const WIN_W: usize = 800;
const WIN_H: usize = 620;

// NES color palette
const NES_BG: u32 = 0x000000;        // Black background
const NES_BORDER: u32 = 0x6888FC;    // Light blue border
const NES_PANEL_BG: u32 = 0x0000A8;  // Dark blue panel
const NES_FIELD_BG: u32 = 0x000000;  // Black playfield
const NES_TEXT: u32 = 0xFCFCFC;      // White text
const NES_SCORE_COLOR: u32 = 0xFCFCFC;

const TETROMINOES: [[[(i32, i32); 4]; 4]; 7] = [
    [[(0,0),(0,1),(0,2),(0,3)],[(0,0),(1,0),(2,0),(3,0)],[(0,0),(0,1),(0,2),(0,3)],[(0,0),(1,0),(2,0),(3,0)]],
    [[(0,0),(0,1),(1,0),(1,1)],[(0,0),(0,1),(1,0),(1,1)],[(0,0),(0,1),(1,0),(1,1)],[(0,0),(0,1),(1,0),(1,1)]],
    [[(0,0),(0,1),(0,2),(1,1)],[(0,0),(1,0),(2,0),(1,1)],[(1,0),(1,1),(1,2),(0,1)],[(0,1),(1,0),(1,1),(2,1)]],
    [[(0,1),(0,2),(1,0),(1,1)],[(0,0),(1,0),(1,1),(2,1)],[(0,1),(0,2),(1,0),(1,1)],[(0,0),(1,0),(1,1),(2,1)]],
    [[(0,0),(0,1),(1,1),(1,2)],[(0,1),(1,0),(1,1),(2,0)],[(0,0),(0,1),(1,1),(1,2)],[(0,1),(1,0),(1,1),(2,0)]],
    [[(0,0),(0,1),(0,2),(1,0)],[(0,0),(1,0),(2,0),(2,1)],[(1,0),(1,1),(1,2),(0,2)],[(0,0),(0,1),(1,1),(2,1)]],
    [[(0,0),(0,1),(0,2),(1,2)],[(0,0),(1,0),(2,0),(0,1)],[(0,0),(1,0),(1,1),(1,2)],[(2,0),(0,1),(1,1),(2,1)]],
];

// NES Tetris Level 0 colors (two-tone per piece)
const COLORS: [u32; 7] = [
    0x009CDC, // I - teal
    0xFCFCFC, // O - white
    0xA800A8, // T - purple
    0x00A800, // S - green
    0xD82800, // Z - red
    0x0058F8, // L - blue
    0x0058F8, // J - blue
];
const COLORS_LIGHT: [u32; 7] = [
    0x6888FC, // I - light blue
    0xD8D8D8, // O - light gray
    0xF878F8, // T - pink
    0xB8F818, // S - lime
    0xF87858, // Z - salmon
    0x6888FC, // L - light blue
    0x6888FC, // J - light blue
];

struct Bag {
    pieces: Vec<usize>,
}

impl Bag {
    fn new() -> Self {
        let mut b = Bag { pieces: vec![] };
        b.refill();
        b
    }

    fn refill(&mut self) {
        let mut set = vec![0, 1, 2, 3, 4, 5, 6];
        let mut rng = rand::thread_rng();
        // Fisher-Yates shuffle
        for i in (1..7).rev() {
            let j = rng.gen_range(0..=i);
            set.swap(i, j);
        }
        self.pieces.extend(set);
    }

    fn next(&mut self) -> usize {
        if self.pieces.is_empty() { self.refill(); }
        self.pieces.remove(0)
    }

    fn peek(&self) -> usize {
        self.pieces[0]
    }
}

struct Game {
    board: [[Option<usize>; BOARD_W]; BOARD_H],
    piece: usize, rot: usize, row: i32, col: i32,
    next: usize,
    bag: Bag,
    score: u32, lines: u32, level: u32,
    game_over: bool,
    auto_play: bool,
    ai_target: Option<(i32, usize)>,
    ai_timer: f64,
    tetris_flash: f64, // countdown for tetris effect (seconds)
}

impl Game {
    fn new() -> Self {
        let mut bag = Bag::new();
        let first = bag.next();
        let next = bag.next();
        let mut g = Game {
            board: [[None; BOARD_W]; BOARD_H],
            piece: first, rot: 0, row: 0, col: 0,
            next,
            bag,
            score: 0, lines: 0, level: 1, game_over: false,
            auto_play: false,
            ai_target: None,
            ai_timer: 0.0,
            tetris_flash: 0.0,
        };
        g.col = BOARD_W as i32 / 2 - 1;
        g
    }

    fn spawn(&mut self) {
        self.piece = self.next;
        self.next = self.bag.next();
        self.rot = 0;
        self.row = 0;
        self.col = BOARD_W as i32 / 2 - 1;
        self.ai_target = None;
        if !self.valid(self.row, self.col, self.rot) { self.game_over = true; }
    }

    fn cells(&self, r: i32, c: i32, rot: usize) -> [(i32, i32); 4] {
        let o = TETROMINOES[self.piece][rot];
        [(r+o[0].0,c+o[0].1),(r+o[1].0,c+o[1].1),(r+o[2].0,c+o[2].1),(r+o[3].0,c+o[3].1)]
    }

    fn cur(&self) -> [(i32, i32); 4] { self.cells(self.row, self.col, self.rot) }

    fn valid(&self, r: i32, c: i32, rot: usize) -> bool {
        for (rr, cc) in self.cells(r, c, rot) {
            if cc < 0 || cc >= BOARD_W as i32 || rr >= BOARD_H as i32 { return false; }
            if rr >= 0 && self.board[rr as usize][cc as usize].is_some() { return false; }
        }
        true
    }

    fn move_lr(&mut self, d: i32) { if self.valid(self.row, self.col+d, self.rot) { self.col += d; } }

    fn rotate(&mut self) {
        let nr = (self.rot + 1) % 4;
        for &off in &[0,1,-1,2,-2] {
            if self.valid(self.row, self.col+off, nr) { self.rot = nr; self.col += off; return; }
        }
    }

    fn down(&mut self) -> bool {
        if self.valid(self.row+1, self.col, self.rot) { self.row += 1; true }
        else { self.lock(); false }
    }

    fn hard_drop(&mut self) {
        while self.valid(self.row+1, self.col, self.rot) { self.row += 1; self.score += 2; }
        self.lock();
    }

    fn ghost(&self) -> i32 {
        let mut r = self.row;
        while self.valid(r+1, self.col, self.rot) { r += 1; }
        r
    }

    fn lock(&mut self) {
        for (r, cc) in self.cur() { if r >= 0 { self.board[r as usize][cc as usize] = Some(self.piece); } }
        let mut cleared = 0u32;
        let mut row = BOARD_H as i32 - 1;
        while row >= 0 {
            if self.board[row as usize].iter().all(|c| c.is_some()) {
                for r in (1..=row as usize).rev() { self.board[r] = self.board[r-1]; }
                self.board[0] = [None; BOARD_W];
                cleared += 1;
            } else { row -= 1; }
        }
        if cleared > 0 {
            self.lines += cleared;
            self.score += match cleared { 1=>100,2=>300,3=>500,4=>800,_=>0 } * self.level;
            self.level = self.lines / 10 + 1;
            if cleared >= 4 { self.tetris_flash = 1.0; }
        }
        self.spawn();
    }

    fn drop_ms(&self) -> u64 {
        match self.level { 1=>800,2=>720,3=>630,4=>550,5=>470,6=>380,7=>300,8=>220,9=>140,_=>80 }
    }

    // AI: evaluate a board position (lower = better)
    fn ai_evaluate(board: &[[Option<usize>; BOARD_W]; BOARD_H]) -> f64 {
        let mut score = 0.0;

        // Heights per column
        let mut heights = [0i32; BOARD_W];
        for c in 0..BOARD_W {
            for r in 0..BOARD_H {
                if board[r][c].is_some() { heights[c] = (BOARD_H - r) as i32; break; }
            }
        }
        let agg_height: i32 = heights.iter().sum();
        score += agg_height as f64 * 0.51;

        // Complete lines — strong Tetris preference
        let complete = (0..BOARD_H).filter(|&r| board[r].iter().all(|c| c.is_some())).count();
        score -= match complete {
            4 => 12.0,
            3 => 2.0,
            2 => 1.2,
            1 => 0.5,
            _ => 0.0,
        };

        // Holes — most important to avoid
        let mut holes = 0;
        for c in 0..BOARD_W {
            let mut found_block = false;
            for r in 0..BOARD_H {
                if board[r][c].is_some() { found_block = true; }
                else if found_block { holes += 1; }
            }
        }
        score += holes as f64 * 0.8;

        // Bumpiness
        let bump: i32 = heights.windows(2).map(|w| (w[0] - w[1]).abs()).sum();
        score += bump as f64 * 0.18;

        // Well preference: reward keeping one edge column low for I-pieces
        let min_h = *heights.iter().min().unwrap();
        let min_col = heights.iter().position(|&h| h == min_h).unwrap();
        if (min_col == 0 || min_col == BOARD_W - 1) && holes == 0 {
            let neighbor = if min_col == 0 { heights[1] } else { heights[BOARD_W - 2] };
            let depth = neighbor - min_h;
            if depth >= 3 { score -= 1.5; }
            else if depth >= 2 { score -= 0.8; }
        }
        // Penalize multiple low columns (messy board)
        let low_count = heights.iter().filter(|&&h| h <= min_h + 1).count();
        if low_count > 2 { score += (low_count - 2) as f64 * 0.3; }

        score
    }

    fn ai_find_best(&self) -> (i32, usize) {
        let mut best_score = f64::MAX;
        let mut best = (self.col, self.rot);

        for rot in 0..4 {
            // Find valid column range for this rotation
            for col in -2..BOARD_W as i32 + 2 {
                if !self.valid(0, col, rot) { continue; }
                // Drop piece
                let mut r = 0;
                while self.valid_for_piece(self.piece, r + 1, col, rot) { r += 1; }
                // Simulate placement
                let mut board = self.board;
                let o = TETROMINOES[self.piece][rot];
                let mut ok = true;
                for &(dr, dc) in &o {
                    let pr = r + dr;
                    let pc = col + dc;
                    if pr < 0 || pr >= BOARD_H as i32 || pc < 0 || pc >= BOARD_W as i32 { ok = false; break; }
                    board[pr as usize][pc as usize] = Some(0);
                }
                if !ok { continue; }
                let s = Self::ai_evaluate(&board);
                if s < best_score { best_score = s; best = (col, rot); }
            }
        }
        best
    }

    fn valid_for_piece(&self, piece: usize, row: i32, col: i32, rot: usize) -> bool {
        let o = TETROMINOES[piece][rot];
        for &(dr, dc) in &o {
            let r = row + dr;
            let c = col + dc;
            if c < 0 || c >= BOARD_W as i32 || r >= BOARD_H as i32 { return false; }
            if r >= 0 && self.board[r as usize][c as usize].is_some() { return false; }
        }
        true
    }
}

fn set_pixel(buf: &mut [u32], x: usize, y: usize, c: u32) {
    if x < WIN_W && y < WIN_H { buf[y * WIN_W + x] = c; }
}

fn fill_rect(buf: &mut [u32], x: usize, y: usize, w: usize, h: usize, c: u32) {
    for dy in 0..h { for dx in 0..w { set_pixel(buf, x+dx, y+dy, c); } }
}

fn draw_rect_outline(buf: &mut [u32], x: usize, y: usize, w: usize, h: usize, c: u32) {
    for dx in 0..w { set_pixel(buf, x+dx, y, c); set_pixel(buf, x+dx, y+h-1, c); }
    for dy in 0..h { set_pixel(buf, x, y+dy, c); set_pixel(buf, x+w-1, y+dy, c); }
}

fn draw_block(buf: &mut [u32], x: usize, y: usize, s: usize, c: u32) {
    // NES-style 3D block
    let light = blend(c, 0xFFFFFF, 100);
    let dark = blend(c, 0x000000, 100);
    // Fill
    fill_rect(buf, x, y, s, s, c);
    // Top and left highlight
    fill_rect(buf, x, y, s, 2, light);
    fill_rect(buf, x, y, 2, s, light);
    // Bottom and right shadow
    fill_rect(buf, x, y + s - 2, s, 2, dark);
    fill_rect(buf, x + s - 2, y, 2, s, dark);
    // Inner square accent
    if s >= 10 {
        fill_rect(buf, x + 4, y + 4, s - 8, s - 8, light);
    }
}

fn draw_nes_block(buf: &mut [u32], x: usize, y: usize, s: usize, piece: usize) {
    let c = COLORS[piece];
    let cl = COLORS_LIGHT[piece];
    let dark = blend(c, 0x000000, 120);
    // Fill with main color
    fill_rect(buf, x, y, s, s, c);
    // Top and left highlight (light color)
    fill_rect(buf, x, y, s, 2, cl);
    fill_rect(buf, x, y, 2, s, cl);
    // Bottom and right shadow
    fill_rect(buf, x, y + s - 2, s, 2, dark);
    fill_rect(buf, x + s - 2, y, 2, s, dark);
    // Inner shine
    if s >= 12 {
        fill_rect(buf, x + 5, y + 5, s - 10, s - 10, cl);
        fill_rect(buf, x + 7, y + 7, s - 14, s - 14, c);
    }
}

fn blend(base: u32, top: u32, alpha: u32) -> u32 {
    let br = (base >> 16) & 0xFF; let bg = (base >> 8) & 0xFF; let bb = base & 0xFF;
    let tr = (top >> 16) & 0xFF; let tg = (top >> 8) & 0xFF; let tb = top & 0xFF;
    let r = (br * (255-alpha) + tr * alpha) / 255;
    let g = (bg * (255-alpha) + tg * alpha) / 255;
    let b = (bb * (255-alpha) + tb * alpha) / 255;
    (r << 16) | (g << 8) | b
}

// Simple 5x7 bitmap font
const FONT: &[(char, [u8; 7])] = &[
    ('0', [0b01110,0b10001,0b10011,0b10101,0b11001,0b10001,0b01110]),
    ('1', [0b00100,0b01100,0b00100,0b00100,0b00100,0b00100,0b01110]),
    ('2', [0b01110,0b10001,0b00001,0b00110,0b01000,0b10000,0b11111]),
    ('3', [0b01110,0b10001,0b00001,0b00110,0b00001,0b10001,0b01110]),
    ('4', [0b00010,0b00110,0b01010,0b10010,0b11111,0b00010,0b00010]),
    ('5', [0b11111,0b10000,0b11110,0b00001,0b00001,0b10001,0b01110]),
    ('6', [0b00110,0b01000,0b10000,0b11110,0b10001,0b10001,0b01110]),
    ('7', [0b11111,0b00001,0b00010,0b00100,0b01000,0b01000,0b01000]),
    ('8', [0b01110,0b10001,0b10001,0b01110,0b10001,0b10001,0b01110]),
    ('9', [0b01110,0b10001,0b10001,0b01111,0b00001,0b00010,0b01100]),
    ('A', [0b01110,0b10001,0b10001,0b11111,0b10001,0b10001,0b10001]),
    ('B', [0b11110,0b10001,0b10001,0b11110,0b10001,0b10001,0b11110]),
    ('C', [0b01110,0b10001,0b10000,0b10000,0b10000,0b10001,0b01110]),
    ('D', [0b11110,0b10001,0b10001,0b10001,0b10001,0b10001,0b11110]),
    ('E', [0b11111,0b10000,0b10000,0b11110,0b10000,0b10000,0b11111]),
    ('F', [0b11111,0b10000,0b10000,0b11110,0b10000,0b10000,0b10000]),
    ('G', [0b01110,0b10001,0b10000,0b10111,0b10001,0b10001,0b01110]),
    ('H', [0b10001,0b10001,0b10001,0b11111,0b10001,0b10001,0b10001]),
    ('I', [0b01110,0b00100,0b00100,0b00100,0b00100,0b00100,0b01110]),
    ('J', [0b00111,0b00010,0b00010,0b00010,0b00010,0b10010,0b01100]),
    ('K', [0b10001,0b10010,0b10100,0b11000,0b10100,0b10010,0b10001]),
    ('L', [0b10000,0b10000,0b10000,0b10000,0b10000,0b10000,0b11111]),
    ('M', [0b10001,0b11011,0b10101,0b10101,0b10001,0b10001,0b10001]),
    ('N', [0b10001,0b11001,0b10101,0b10011,0b10001,0b10001,0b10001]),
    ('O', [0b01110,0b10001,0b10001,0b10001,0b10001,0b10001,0b01110]),
    ('P', [0b11110,0b10001,0b10001,0b11110,0b10000,0b10000,0b10000]),
    ('Q', [0b01110,0b10001,0b10001,0b10001,0b10101,0b10010,0b01101]),
    ('R', [0b11110,0b10001,0b10001,0b11110,0b10100,0b10010,0b10001]),
    ('S', [0b01110,0b10001,0b10000,0b01110,0b00001,0b10001,0b01110]),
    ('T', [0b11111,0b00100,0b00100,0b00100,0b00100,0b00100,0b00100]),
    ('U', [0b10001,0b10001,0b10001,0b10001,0b10001,0b10001,0b01110]),
    ('V', [0b10001,0b10001,0b10001,0b10001,0b10001,0b01010,0b00100]),
    ('W', [0b10001,0b10001,0b10001,0b10101,0b10101,0b10101,0b01010]),
    ('X', [0b10001,0b10001,0b01010,0b00100,0b01010,0b10001,0b10001]),
    ('Y', [0b10001,0b10001,0b01010,0b00100,0b00100,0b00100,0b00100]),
    ('Z', [0b11111,0b00001,0b00010,0b00100,0b01000,0b10000,0b11111]),
    (' ', [0b00000,0b00000,0b00000,0b00000,0b00000,0b00000,0b00000]),
    (':', [0b00000,0b00100,0b00100,0b00000,0b00100,0b00100,0b00000]),
    ('/', [0b00001,0b00010,0b00010,0b00100,0b01000,0b01000,0b10000]),
    ('<', [0b00010,0b00100,0b01000,0b10000,0b01000,0b00100,0b00010]),
    ('>', [0b01000,0b00100,0b00010,0b00001,0b00010,0b00100,0b01000]),
    ('=', [0b00000,0b00000,0b11111,0b00000,0b11111,0b00000,0b00000]),
    ('-', [0b00000,0b00000,0b00000,0b11111,0b00000,0b00000,0b00000]),
    ('!', [0b00100,0b00100,0b00100,0b00100,0b00100,0b00000,0b00100]),
    ('.', [0b00000,0b00000,0b00000,0b00000,0b00000,0b00000,0b00100]),
];

fn draw_char(buf: &mut [u32], ch: char, x: usize, y: usize, scale: usize, color: u32) {
    let upper = ch.to_ascii_uppercase();
    let glyph = FONT.iter().find(|(c, _)| *c == upper).map(|(_, g)| g);
    if let Some(g) = glyph {
        for (row, &bits) in g.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    fill_rect(buf, x + col * scale, y + row * scale, scale, scale, color);
                }
            }
        }
    }
}

fn draw_text(buf: &mut [u32], text: &str, x: usize, y: usize, scale: usize, color: u32) {
    for (i, ch) in text.chars().enumerate() {
        draw_char(buf, ch, x + i * 6 * scale, y, scale, color);
    }
}

fn draw_button(buf: &mut [u32], x: usize, y: usize, w: usize, h: usize, label: &str, hover: bool) {
    let bg = if hover { 0x0058F8 } else { NES_PANEL_BG };
    fill_rect(buf, x, y, w, h, bg);
    draw_rect_outline(buf, x, y, w, h, NES_BORDER);
    let tw = label.len() * 12;
    let tx = x + (w.saturating_sub(tw)) / 2;
    let ty = y + (h.saturating_sub(14)) / 2;
    draw_text(buf, label, tx, ty, 2, NES_TEXT);
}

fn main() {
    let mut window = Window::new("Tetris", WIN_W, WIN_H, WindowOptions::default())
        .expect("Failed to create window");
    window.set_target_fps(60);

    let mut buf = vec![0u32; WIN_W * WIN_H];
    let mut game = Game::new();
    let mut last_drop = Instant::now();
    let mut prev_keys: Vec<Key> = vec![];
    let mut frame: u64 = 0;
    // Generate random star positions
    let mut rng = rand::thread_rng();
    let stars: Vec<(usize, usize, u32)> = (0..40).map(|_| {
        (rng.gen_range(0..WIN_W), rng.gen_range(0..WIN_H / 2 + 100), rng.gen_range(0..360))
    }).collect();

    loop {
        if !window.is_open() || window.is_key_down(Key::Escape) { break; }

        let keys = window.get_keys();
        let newly_pressed: Vec<Key> = keys.iter().filter(|k| !prev_keys.contains(k)).copied().collect();

        // Mouse
        let mouse_pos = window.get_mouse_pos(MouseMode::Clamp);
        let mouse_down = window.get_mouse_down(MouseButton::Left);

        // Toggle auto-play
        for &k in &newly_pressed {
            if k == Key::T { game.auto_play = !game.auto_play; game.ai_target = None; }
        }

        if !game.game_over {
            if game.auto_play {
                // AI auto-play
                if game.ai_target.is_none() {
                    game.ai_target = Some(game.ai_find_best());
                }
                game.ai_timer += 1.0 / 60.0;
                let ai_speed = 0.005; // seconds per AI move
                if game.ai_timer >= ai_speed {
                    game.ai_timer = 0.0;
                    if let Some((target_col, target_rot)) = game.ai_target {
                        if game.rot != target_rot {
                            game.rotate();
                        } else if game.col < target_col {
                            game.move_lr(1);
                        } else if game.col > target_col {
                            game.move_lr(-1);
                        } else {
                            game.hard_drop();
                        }
                    }
                }
            } else {
                for &k in &newly_pressed {
                    match k {
                        Key::Left | Key::A => game.move_lr(-1),
                        Key::Right | Key::D => game.move_lr(1),
                        Key::Up | Key::W => game.rotate(),
                        Key::Space => game.hard_drop(),
                        Key::Down | Key::S => { game.down(); game.score += 1; }
                        _ => {}
                    }
                }
            }

            let interval = if !game.auto_play && (keys.contains(&Key::Down) || keys.contains(&Key::S)) {
                game.drop_ms() / 10
            } else {
                game.drop_ms()
            };

            if last_drop.elapsed().as_millis() as u64 >= interval {
                game.down();
                last_drop = Instant::now();
            }
        } else {
            for &k in &newly_pressed {
                if k == Key::R { game = Game::new(); last_drop = Instant::now(); }
            }
        }

        //

        // === DRAW (NES STYLE) ===
        buf.fill(NES_BG);
        frame += 1;

        // --- Background: Night sky with twinkling stars ---
        for (i, &(sx, sy, phase)) in stars.iter().enumerate() {
            let t = ((frame as f64 / 30.0) + phase as f64 / 60.0).sin();
            let brightness = ((t * 0.5 + 0.5) * 255.0) as u32;
            let star_c = (brightness << 16) | (brightness << 8) | brightness;
            set_pixel(&mut buf, sx, sy, star_c);
            // Some stars are bigger
            if i % 3 == 0 {
                set_pixel(&mut buf, sx + 1, sy, star_c);
                set_pixel(&mut buf, sx, sy + 1, star_c);
            }
        }

        // --- St. Basil's Cathedral (detailed pixel art) ---
        let ground_y = WIN_H - 35;
        let cat_x = 170i32; // Left side, visible area

        // Night sky gradient near horizon
        for y in (ground_y.saturating_sub(120))..ground_y {
            let t = (y as i32 - (ground_y as i32 - 120)).max(0) as u32;
            let r = t / 8;
            let g = t / 6;
            let b = (t / 3).min(40);
            let c = (r << 16) | (g << 8) | b;
            for x in 0..WIN_W { set_pixel(&mut buf, x, y, c); }
        }

        // Ground - Red Square cobblestone
        fill_rect(&mut buf, 0, ground_y, WIN_W, WIN_H - ground_y, 0x1A1008);
        // Cobblestone texture
        for i in 0..30 {
            let cx = (i * 23 + 7) % WIN_W;
            let cy = ground_y + (i * 7 + 3) % (WIN_H - ground_y);
            fill_rect(&mut buf, cx, cy, 8, 3, 0x221810);
        }
        // Ground line
        fill_rect(&mut buf, 0, ground_y, WIN_W, 2, 0x383028);

        // === Helper: draw striped onion dome ===
        let draw_onion = |buf: &mut [u32], cx: i32, base_y: i32, w: i32, h: i32,
                          c1: u32, c2: u32, stripe_w: i32, gold_tip: bool| {
            // Gold cross on top
            let cross_c = 0xD8B800;
            let tip_y = base_y - h - 18;
            fill_rect(buf, (cx - 1) as usize, tip_y as usize, 2, 14, cross_c);
            fill_rect(buf, (cx - 4) as usize, (tip_y + 4) as usize, 8, 2, cross_c);
            // Gold ball at tip
            if gold_tip {
                fill_rect(buf, (cx - 2) as usize, (tip_y - 3) as usize, 5, 5, cross_c);
                fill_rect(buf, (cx - 1) as usize, (tip_y - 4) as usize, 3, 7, cross_c);
            }

            // Onion dome shape - draw row by row for smooth curve
            for dy in 0..h {
                let t = dy as f64 / h as f64; // 0 at top, 1 at bottom
                // Onion curve: narrow tip, bulge out, narrow at base
                let width = if t < 0.15 {
                    // Narrow tip
                    (t / 0.15 * w as f64 * 0.2) as i32
                } else if t < 0.5 {
                    // Expanding bulge
                    let bt = (t - 0.15) / 0.35;
                    (w as f64 * (0.2 + bt * 0.8)) as i32
                } else if t < 0.8 {
                    // Maximum width
                    w
                } else {
                    // Taper at bottom
                    let bt = (t - 0.8) / 0.2;
                    (w as f64 * (1.0 - bt * 0.3)) as i32
                };

                let y = (base_y - h + dy) as usize;
                let x_start = cx - width / 2;
                for dx in 0..width {
                    let x = (x_start + dx) as usize;
                    // Striped pattern
                    let stripe = if stripe_w > 0 {
                        // Vertical spiral stripes (shift with row for spiral effect)
                        ((dx + dy / 3) / stripe_w) % 2 == 0
                    } else {
                        // Horizontal stripes
                        (dy / 3) % 2 == 0
                    };
                    let color = if stripe { c1 } else { c2 };
                    if x < WIN_W && y < WIN_H {
                        set_pixel(buf, x, y, color);
                    }
                }
            }
        };

        // Brick color palette
        let brick = 0xB85838;      // Main brick
        let brick_dark = 0x883828; // Dark brick
        let brick_lite = 0xD07050; // Light brick
        let white_trim = 0xE8E0D0; // White/cream trim
        let green_roof = 0x286828; // Green roof

        // === MAIN CENTRAL TOWER (tallest) ===
        // Multi-tiered tower body
        let ct_x = cat_x;
        let ct_base = ground_y as i32;

        // Base tier (wide)
        fill_rect(&mut buf, (ct_x - 35) as usize, (ct_base - 60) as usize, 70, 60, brick);
        // White trim bands
        fill_rect(&mut buf, (ct_x - 36) as usize, (ct_base - 60) as usize, 72, 3, white_trim);
        fill_rect(&mut buf, (ct_x - 36) as usize, (ct_base - 30) as usize, 72, 2, white_trim);
        // Windows on base
        for wx in [-20i32, -5, 10] {
            fill_rect(&mut buf, (ct_x + wx) as usize, (ct_base - 52) as usize, 8, 16, 0x181008);
            // Arch top
            fill_rect(&mut buf, (ct_x + wx + 1) as usize, (ct_base - 54) as usize, 6, 2, brick_lite);
        }

        // Second tier (narrower, octagonal look)
        fill_rect(&mut buf, (ct_x - 22) as usize, (ct_base - 110) as usize, 44, 50, brick_lite);
        fill_rect(&mut buf, (ct_x - 24) as usize, (ct_base - 110) as usize, 48, 3, white_trim);
        fill_rect(&mut buf, (ct_x - 24) as usize, (ct_base - 80) as usize, 48, 2, white_trim);
        // Kokoshnik arches (decorative)
        for kx in [-16i32, -4, 8] {
            fill_rect(&mut buf, (ct_x + kx) as usize, (ct_base - 106) as usize, 10, 2, 0xD8B800);
            fill_rect(&mut buf, (ct_x + kx + 2) as usize, (ct_base - 108) as usize, 6, 2, 0xD8B800);
        }
        // Windows
        for wx in [-14i32, 4] {
            fill_rect(&mut buf, (ct_x + wx) as usize, (ct_base - 100) as usize, 6, 12, 0x181008);
        }

        // Third tier (spire section)
        fill_rect(&mut buf, (ct_x - 14) as usize, (ct_base - 155) as usize, 28, 45, brick);
        fill_rect(&mut buf, (ct_x - 16) as usize, (ct_base - 155) as usize, 32, 2, white_trim);
        // Zigzag decoration
        for i in 0..8 {
            let zx = ct_x - 12 + i * 3;
            fill_rect(&mut buf, zx as usize, (ct_base - 150) as usize, 2, 2, 0xD8B800);
            fill_rect(&mut buf, (zx + 1) as usize, (ct_base - 148) as usize, 2, 2, 0xD8B800);
        }

        // Pointed spire top
        for dy in 0..30 {
            let w = (30 - dy) * 14 / 30;
            fill_rect(&mut buf, (ct_x - w / 2) as usize, (ct_base - 185 + dy) as usize,
                      w as usize, 1, brick_lite);
            // Red and white alternating pattern
            if dy % 4 < 2 {
                fill_rect(&mut buf, (ct_x - w / 2) as usize, (ct_base - 185 + dy) as usize,
                          w as usize, 1, 0xD07050);
            }
        }

        // GOLD dome on top of central spire
        draw_onion(&mut buf, ct_x, ct_base - 185, 14, 20, 0xD8B800, 0xC0A000, 0, true);

        // === LEFT DOME 1 - Green & Gold striped (large) ===
        let ld1_x = ct_x - 60;
        fill_rect(&mut buf, (ld1_x - 16) as usize, (ct_base - 75) as usize, 32, 75, brick_dark);
        fill_rect(&mut buf, (ld1_x - 18) as usize, (ct_base - 75) as usize, 36, 3, white_trim);
        // Arched windows
        for wx in [-8i32, 4] {
            fill_rect(&mut buf, (ld1_x + wx) as usize, (ct_base - 65) as usize, 6, 10, 0x181008);
        }
        draw_onion(&mut buf, ld1_x, ct_base - 75, 26, 36, 0x286828, 0xC8B020, 4, true);

        // === LEFT DOME 2 - Blue & White striped ===
        let ld2_x = ct_x - 25;
        fill_rect(&mut buf, (ld2_x - 14) as usize, (ct_base - 90) as usize, 28, 90, brick);
        fill_rect(&mut buf, (ld2_x - 16) as usize, (ct_base - 90) as usize, 32, 2, white_trim);
        fill_rect(&mut buf, (ld2_x - 16) as usize, (ct_base - 60) as usize, 32, 2, white_trim);
        for wx in [-8i32, 2] {
            fill_rect(&mut buf, (ld2_x + wx) as usize, (ct_base - 82) as usize, 6, 12, 0x181008);
        }
        draw_onion(&mut buf, ld2_x, ct_base - 90, 22, 30, 0x2050C8, 0xE8E8F0, 3, true);

        // === RIGHT DOME 1 - Red & Green checkered ===
        let rd1_x = ct_x + 55;
        fill_rect(&mut buf, (rd1_x - 16) as usize, (ct_base - 80) as usize, 32, 80, brick);
        fill_rect(&mut buf, (rd1_x - 18) as usize, (ct_base - 80) as usize, 36, 3, white_trim);
        for wx in [-8i32, 4] {
            fill_rect(&mut buf, (rd1_x + wx) as usize, (ct_base - 70) as usize, 6, 10, 0x181008);
        }
        draw_onion(&mut buf, rd1_x, ct_base - 80, 24, 32, 0xC82020, 0x206828, 3, true);

        // === RIGHT DOME 2 - Gold/Yellow patterned ===
        let rd2_x = ct_x + 28;
        fill_rect(&mut buf, (rd2_x - 12) as usize, (ct_base - 85) as usize, 24, 85, brick_lite);
        fill_rect(&mut buf, (rd2_x - 14) as usize, (ct_base - 85) as usize, 28, 2, white_trim);
        for wx in [-6i32, 2] {
            fill_rect(&mut buf, (rd2_x + wx) as usize, (ct_base - 78) as usize, 5, 10, 0x181008);
        }
        draw_onion(&mut buf, rd2_x, ct_base - 85, 18, 24, 0xC8A020, 0x388830, 3, true);

        // === FAR LEFT small dark tower ===
        let fl_x = ct_x - 90;
        fill_rect(&mut buf, (fl_x - 10) as usize, (ct_base - 55) as usize, 20, 55, brick_dark);
        fill_rect(&mut buf, (fl_x - 12) as usize, (ct_base - 55) as usize, 24, 2, white_trim);
        // Small pointed roof
        for dy in 0..20 {
            let w = (20 - dy) * 12 / 20;
            let c = if dy % 3 == 0 { 0x183018 } else { 0x284828 };
            fill_rect(&mut buf, (fl_x - w / 2) as usize, (ct_base - 75 + dy) as usize, w as usize, 1, c);
        }
        // Tiny dome
        draw_onion(&mut buf, fl_x, ct_base - 75, 10, 14, 0x2030A0, 0x602060, 2, true);

        // === FAR RIGHT small tower ===
        let fr_x = ct_x + 82;
        fill_rect(&mut buf, (fr_x - 10) as usize, (ct_base - 50) as usize, 20, 50, brick_dark);
        fill_rect(&mut buf, (fr_x - 12) as usize, (ct_base - 50) as usize, 24, 2, white_trim);
        // Green roof
        for dy in 0..15 {
            let w = (15 - dy) * 14 / 15;
            fill_rect(&mut buf, (fr_x - w / 2) as usize, (ct_base - 65 + dy) as usize, w as usize, 1, green_roof);
        }

        // === Base wall connecting everything ===
        fill_rect(&mut buf, (ct_x - 95) as usize, (ct_base - 18) as usize, 190, 18, brick);
        fill_rect(&mut buf, (ct_x - 96) as usize, (ct_base - 18) as usize, 192, 2, white_trim);
        fill_rect(&mut buf, (ct_x - 96) as usize, (ct_base - 1) as usize, 192, 2, white_trim);
        // Archways in base wall
        for ax in [-60i32, -20, 20, 50] {
            fill_rect(&mut buf, (ct_x + ax) as usize, (ct_base - 16) as usize, 14, 16, 0x100808);
            fill_rect(&mut buf, (ct_x + ax + 1) as usize, (ct_base - 18) as usize, 12, 2, brick_lite);
            fill_rect(&mut buf, (ct_x + ax + 2) as usize, (ct_base - 20) as usize, 10, 2, brick_lite);
            fill_rect(&mut buf, (ct_x + ax + 3) as usize, (ct_base - 22) as usize, 8, 2, brick_lite);
        }

        // Animated window glow
        for wx in [-60i32, -25, 28, 55] {
            let glow = (((frame as f64 / 40.0) + wx as f64 / 10.0).sin() * 0.3 + 0.7);
            let gi = (glow * 60.0) as u32;
            let gc = (gi << 16) | ((gi * 9 / 10) << 8) | (gi / 3);
            fill_rect(&mut buf, (ct_x + wx + 3) as usize, (ct_base - 12) as usize, 8, 8, gc);
        }

        // Snowflakes animation - heavy snowfall
        for i in 0..80 {
            // Each snowflake has unique speed, drift and size
            let speed = 0.4 + (i as f64 * 0.618).fract() * 0.8; // varied fall speed
            let drift = ((frame as f64 * 0.02 + i as f64 * 1.7).sin()) * 30.0; // horizontal sway
            let sx = ((i as f64 * 51.7 + drift) % WIN_W as f64 + WIN_W as f64) as usize % WIN_W;
            let sy = ((frame as f64 * speed + i as f64 * 41.3) % (ground_y as f64 + 20.0)) as usize;
            let brightness = (((frame as f64 / 25.0 + i as f64).sin() * 0.2 + 0.8) * 255.0) as u32;
            let sc = (brightness << 16) | (brightness << 8) | brightness;
            if sy < WIN_H {
                set_pixel(&mut buf, sx, sy, sc);
                // Larger flakes for some
                if i % 3 == 0 && sx + 1 < WIN_W {
                    set_pixel(&mut buf, sx + 1, sy, sc);
                    if sy + 1 < WIN_H { set_pixel(&mut buf, sx, sy + 1, sc); }
                }
                // Even bigger flakes
                if i % 7 == 0 && sx + 2 < WIN_W && sy + 2 < WIN_H {
                    set_pixel(&mut buf, sx + 1, sy + 1, sc);
                    set_pixel(&mut buf, sx + 2, sy, sc);
                    set_pixel(&mut buf, sx, sy + 2, sc);
                }
            }
        }
        // Snow accumulation on ground
        for i in 0..WIN_W {
            let h = (((i as f64 * 0.05).sin() * 2.0 + 3.0) as usize).min(6);
            fill_rect(&mut buf, i, ground_y.saturating_sub(h), 1, h, 0xD0D8E0);
        }

        // Title area - with decorative Russian-style borders
        let title_x = BOARD_X + BOARD_W * CELL / 2 - 72;
        let title_y = 10;
        // Ornamental border around title
        fill_rect(&mut buf, title_x - 15, title_y - 4, 180, 34, 0x0000A8);
        draw_rect_outline(&mut buf, title_x - 15, title_y - 4, 180, 34, 0xD8B800);
        draw_rect_outline(&mut buf, title_x - 13, title_y - 2, 176, 30, 0xA81010);
        // Star decorations on title corners
        fill_rect(&mut buf, title_x - 18, title_y - 1, 6, 6, 0xD8B800);
        fill_rect(&mut buf, title_x + 162, title_y - 1, 6, 6, 0xD8B800);
        draw_text(&mut buf, "TETRIS", title_x + 18, title_y + 4, 3, NES_TEXT);

        // Right-side info panels (all on right of board)
        // Lines box at top-right area
        let lp_x = BOARD_X + BOARD_W * CELL + 18;
        let lp_y = BOARD_Y;
        // (lines merged into score panel below)

        // Board border (ornamental Russian style)
        let bx = BOARD_X - 6;
        let by = BOARD_Y - 6;
        let bw = BOARD_W * CELL + 12;
        let bh = BOARD_H * CELL + 12;
        fill_rect(&mut buf, bx, by, bw, bh, NES_PANEL_BG);
        draw_rect_outline(&mut buf, bx, by, bw, bh, 0xD8B800);
        draw_rect_outline(&mut buf, bx + 1, by + 1, bw - 2, bh - 2, 0xA81010);
        draw_rect_outline(&mut buf, bx + 2, by + 2, bw - 4, bh - 4, NES_BORDER);
        // Corner ornaments
        for &(cx, cy) in &[(bx, by), (bx + bw - 6, by), (bx, by + bh - 6), (bx + bw - 6, by + bh - 6)] {
            fill_rect(&mut buf, cx, cy, 6, 6, 0xD8B800);
        }

        // Board field
        fill_rect(&mut buf, BOARD_X, BOARD_Y, BOARD_W * CELL, BOARD_H * CELL, NES_FIELD_BG);

        // Locked cells (NES style)
        for r in 0..BOARD_H {
            for c in 0..BOARD_W {
                if let Some(piece_idx) = game.board[r][c] {
                    draw_nes_block(&mut buf, BOARD_X + c * CELL, BOARD_Y + r * CELL, CELL, piece_idx);
                }
            }
        }

        if !game.game_over {
            // Ghost piece (subtle)
            let gr = game.ghost();
            let gc = game.cells(gr, game.col, game.rot);
            let pc = COLORS[game.piece];
            for (r, c) in gc {
                if r >= 0 {
                    let ghost_c = blend(NES_FIELD_BG, pc, 30);
                    fill_rect(&mut buf, (BOARD_X as i32 + c * CELL as i32 + 2) as usize,
                        (BOARD_Y as i32 + r * CELL as i32 + 2) as usize, CELL-4, CELL-4, ghost_c);
                }
            }

            // Current piece (NES style)
            for (r, c) in game.cur() {
                if r >= 0 {
                    draw_nes_block(&mut buf, (BOARD_X as i32 + c * CELL as i32) as usize,
                        (BOARD_Y as i32 + r * CELL as i32) as usize, CELL, game.piece);
                }
            }
        }

        // Right panel - all info
        let px = BOARD_X + BOARD_W * CELL + 16;
        let pw = 170;

        // Score + Lines box
        let mut py = BOARD_Y;
        fill_rect(&mut buf, px, py, pw, 68, NES_PANEL_BG);
        draw_rect_outline(&mut buf, px, py, pw, 68, 0xD8B800);
        draw_rect_outline(&mut buf, px + 1, py + 1, pw - 2, 66, NES_BORDER);
        draw_text(&mut buf, "SCORE", px + 8, py + 5, 2, 0xD8B800);
        draw_text(&mut buf, &format!("{:06}", game.score), px + 8, py + 22, 3, NES_SCORE_COLOR);
        draw_text(&mut buf, "LINES", px + 8, py + 48, 2, 0xD8B800);
        draw_text(&mut buf, &format!("{:03}", game.lines), px + 80, py + 48, 2, NES_SCORE_COLOR);

        // Next box
        py += 78;
        fill_rect(&mut buf, px, py, pw, 82, NES_PANEL_BG);
        draw_rect_outline(&mut buf, px, py, pw, 82, 0xD8B800);
        draw_rect_outline(&mut buf, px + 1, py + 1, pw - 2, 80, NES_BORDER);
        draw_text(&mut buf, "NEXT", px + 50, py + 5, 2, 0xD8B800);
        fill_rect(&mut buf, px + 20, py + 20, 130, 1, 0xD8B800);
        let no = TETROMINOES[game.next][0];
        for &(dr, dc) in &no {
            draw_nes_block(&mut buf, px + 45 + dc as usize * 20, py + 28 + dr as usize * 20, 20, game.next);
        }

        // Level box
        py += 92;
        fill_rect(&mut buf, px, py, pw, 42, NES_PANEL_BG);
        draw_rect_outline(&mut buf, px, py, pw, 42, 0xD8B800);
        draw_rect_outline(&mut buf, px + 1, py + 1, pw - 2, 40, NES_BORDER);
        draw_text(&mut buf, "LEVEL", px + 8, py + 5, 2, 0xD8B800);
        draw_text(&mut buf, &format!("{:02}", game.level), px + 80, py + 5, 3, NES_SCORE_COLOR);
        draw_text(&mut buf, "TYPE A", px + 8, py + 26, 2, 0x6888FC);

        // Controls box
        py += 52;
        fill_rect(&mut buf, px, py, pw, 82, NES_PANEL_BG);
        draw_rect_outline(&mut buf, px, py, pw, 82, 0xD8B800);
        draw_rect_outline(&mut buf, px + 1, py + 1, pw - 2, 80, NES_BORDER);
        draw_text(&mut buf, "CONTROLS", px + 30, py + 5, 2, 0xD8B800);
        fill_rect(&mut buf, px + 10, py + 4, 4, 4, 0xA81010);
        fill_rect(&mut buf, px + pw - 14, py + 4, 4, 4, 0xA81010);
        draw_text(&mut buf, "ARROWS/WASD", px + 8, py + 22, 2, 0x6888FC);
        draw_text(&mut buf, "SPACE: DROP", px + 8, py + 38, 2, 0x6888FC);
        draw_text(&mut buf, "R:NEW T:AI", px + 8, py + 54, 2, 0x6888FC);
        if game.auto_play {
            draw_text(&mut buf, "AI ON", px + 40, py + 68, 2, 0x00FF88);
        }

        //

        // Tetris flash effect
        if game.tetris_flash > 0.0 {
            game.tetris_flash -= 1.0 / 60.0;
            let intensity = (game.tetris_flash * 3.0).min(1.0);
            // White flash over the board area
            let alpha = (intensity * 180.0) as u32;
            let flash_color = (alpha << 16) | (alpha << 8) | alpha;
            for py in BOARD_Y..BOARD_Y + BOARD_H * CELL {
                for px_i in BOARD_X..BOARD_X + BOARD_W * CELL {
                    if py < WIN_H && px_i < WIN_W {
                        let idx = py * WIN_W + px_i;
                        buf[idx] = blend(buf[idx], 0xFFFFFF, (intensity * 120.0) as u32);
                    }
                }
            }
            // Rainbow "TETRIS!" text
            let t = (1.0 - game.tetris_flash) * 6.0;
            let rainbow = match (t as u32) % 6 {
                0 => 0xFF0000, 1 => 0xFF8800, 2 => 0xFFFF00,
                3 => 0x00FF00, 4 => 0x0088FF, _ => 0xFF00FF,
            };
            let cx = BOARD_X + (BOARD_W * CELL) / 2;
            let cy = BOARD_Y + (BOARD_H * CELL) / 2;
            // Pulsing scale effect
            let scale = 4;
            let tw = 7 * 6 * scale; // "TETRIS!" = 7 chars
            draw_text(&mut buf, "TETRIS!", cx - tw / 2, cy - scale * 4, scale, rainbow);
            // Side sparkle lines
            let sparkle_w = (intensity * BOARD_W as f64 * CELL as f64 * 0.5) as usize;
            let sparkle_y = cy + scale * 5;
            if sparkle_w > 0 {
                fill_rect(&mut buf, cx - sparkle_w / 2, sparkle_y, sparkle_w, 3, rainbow);
                fill_rect(&mut buf, cx - sparkle_w / 2, sparkle_y - 20, sparkle_w, 3, rainbow);
            }
        }

        // Game over overlay
        if game.game_over {
            let cx = BOARD_X + (BOARD_W * CELL) / 2;
            let cy = BOARD_Y + (BOARD_H * CELL) / 2;
            fill_rect(&mut buf, cx - 110, cy - 45, 220, 90, NES_PANEL_BG);
            draw_rect_outline(&mut buf, cx - 110, cy - 45, 220, 90, NES_BORDER);
            draw_rect_outline(&mut buf, cx - 109, cy - 44, 218, 88, NES_BORDER);
            draw_text(&mut buf, "GAME OVER", cx - 80, cy - 28, 3, 0xD82800);
            draw_text(&mut buf, "PRESS R", cx - 50, cy + 12, 2, NES_TEXT);
        }

        window.update_with_buffer(&buf, WIN_W, WIN_H).unwrap();
        prev_keys = keys;
    }
}
