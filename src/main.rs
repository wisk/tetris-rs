use minifb::{Key, Window, WindowOptions};
use rand::Rng;
use rusqlite::Connection;
use std::time::Instant;

const BOARD_W: usize = 10;
const BOARD_H: usize = 20;
const DEFAULT_WIN_W: usize = 800;
const DEFAULT_WIN_H: usize = 620;

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

struct Layout {
    win_w: usize,
    win_h: usize,
    cell: usize,
    board_x: usize,
    board_y: usize,
}

impl Layout {
    fn compute(win_w: usize, win_h: usize) -> Self {
        // Cell size based on window height, leaving room for title and bottom margin
        let cell = ((win_h - 80) / BOARD_H).max(8);
        // Board horizontally: leave left side for cathedral (~45% of width), board, then right panel
        let board_w_px = BOARD_W * cell;
        let right_panel_w = 180;
        // Center the board+panel in the right portion of the window
        let left_area = (win_w * 45) / 100;
        let right_area = win_w - left_area;
        let total_game_w = board_w_px + 16 + right_panel_w;
        let board_x = if right_area >= total_game_w {
            left_area + (right_area - total_game_w) / 2
        } else {
            left_area
        };
        let board_h_px = BOARD_H * cell;
        let board_y = (win_h - board_h_px) / 2;
        Layout { win_w, win_h, cell, board_x, board_y }
    }
}

struct HighScore {
    score: u32,
    _lines: u32,
    level: u32,
    _date: String,
}

fn db_path() -> std::path::PathBuf {
    let mut p = std::env::current_exe().unwrap_or_default();
    p.pop();
    p.push("tetris_scores.db");
    p
}

fn init_db() -> Connection {
    let conn = Connection::open(db_path()).expect("Failed to open score database");
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS high_scores (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            score INTEGER NOT NULL,
            lines INTEGER NOT NULL,
            level INTEGER NOT NULL,
            date TEXT NOT NULL
        )"
    ).expect("Failed to create table");
    conn
}

fn save_score(conn: &Connection, score: u32, lines: u32, level: u32) {
    let date = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    conn.execute(
        "INSERT INTO high_scores (score, lines, level, date) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![score, lines, level, date],
    ).ok();
}

fn get_top_scores(conn: &Connection, limit: usize) -> Vec<HighScore> {
    let mut stmt = conn.prepare(
        "SELECT score, lines, level, date FROM high_scores ORDER BY score DESC LIMIT ?1"
    ).unwrap();
    stmt.query_map(rusqlite::params![limit], |row| {
        Ok(HighScore {
            score: row.get(0)?,
            _lines: row.get(1)?,
            level: row.get(2)?,
            _date: row.get(3)?,
        })
    }).unwrap().filter_map(|r| r.ok()).collect()
}

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

}

struct Game {
    board: [[Option<usize>; BOARD_W]; BOARD_H],
    piece: usize, rot: usize, row: i32, col: i32,
    next: usize,
    bag: Bag,
    score: u32, lines: u32, level: u32,
    game_over: bool,
    score_saved: bool,
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
            score: 0, lines: 0, level: 1, game_over: false, score_saved: false,
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

fn set_pixel(buf: &mut [u32], ww: usize, wh: usize, x: usize, y: usize, c: u32) {
    if x < ww && y < wh { buf[y * ww + x] = c; }
}

fn fill_rect(buf: &mut [u32], ww: usize, wh: usize, x: usize, y: usize, w: usize, h: usize, c: u32) {
    for dy in 0..h { for dx in 0..w { set_pixel(buf, ww, wh, x+dx, y+dy, c); } }
}

fn draw_rect_outline(buf: &mut [u32], ww: usize, wh: usize, x: usize, y: usize, w: usize, h: usize, c: u32) {
    for dx in 0..w { set_pixel(buf, ww, wh, x+dx, y, c); set_pixel(buf, ww, wh, x+dx, y+h-1, c); }
    for dy in 0..h { set_pixel(buf, ww, wh, x, y+dy, c); set_pixel(buf, ww, wh, x+w-1, y+dy, c); }
}

fn draw_nes_block(buf: &mut [u32], ww: usize, wh: usize, x: usize, y: usize, s: usize, piece: usize) {
    let c = COLORS[piece];
    let cl = COLORS_LIGHT[piece];
    let dark = blend(c, 0x000000, 120);
    let b = (s / 12).max(1); // border thickness scales with cell size
    fill_rect(buf, ww, wh, x, y, s, s, c);
    fill_rect(buf, ww, wh, x, y, s, b, cl);
    fill_rect(buf, ww, wh, x, y, b, s, cl);
    fill_rect(buf, ww, wh, x, y + s - b, s, b, dark);
    fill_rect(buf, ww, wh, x + s - b, y, b, s, dark);
    if s >= 12 {
        let i = s / 5;
        let j = s / 4;
        fill_rect(buf, ww, wh, x + i, y + i, s - i*2, s - i*2, cl);
        fill_rect(buf, ww, wh, x + j, y + j, s - j*2, s - j*2, c);
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

fn draw_char(buf: &mut [u32], ww: usize, wh: usize, ch: char, x: usize, y: usize, scale: usize, color: u32) {
    let upper = ch.to_ascii_uppercase();
    let glyph = FONT.iter().find(|(c, _)| *c == upper).map(|(_, g)| g);
    if let Some(g) = glyph {
        for (row, &bits) in g.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    fill_rect(buf, ww, wh, x + col * scale, y + row * scale, scale, scale, color);
                }
            }
        }
    }
}

fn draw_text(buf: &mut [u32], ww: usize, wh: usize, text: &str, x: usize, y: usize, scale: usize, color: u32) {
    for (i, ch) in text.chars().enumerate() {
        draw_char(buf, ww, wh, ch, x + i * 6 * scale, y, scale, color);
    }
}

fn main() {
    let mut window = Window::new("Tetris", DEFAULT_WIN_W, DEFAULT_WIN_H, WindowOptions {
        resize: true,
        ..WindowOptions::default()
    }).expect("Failed to create window");
    window.set_target_fps(60);

    let db = init_db();
    let mut win_w = DEFAULT_WIN_W;
    let mut win_h = DEFAULT_WIN_H;
    let mut buf = vec![0u32; win_w * win_h];
    let mut game = Game::new();
    let mut cached_high_scores: Vec<HighScore> = get_top_scores(&db, 5);
    let mut last_drop = Instant::now();
    let mut prev_keys: Vec<Key> = vec![];
    let mut frame: u64 = 0;
    // Generate random star positions (normalized 0.0..1.0)
    let mut rng = rand::thread_rng();
    let stars: Vec<(f64, f64, u32)> = (0..40).map(|_| {
        (rng.r#gen::<f64>(), rng.r#gen::<f64>() * 0.7, rng.gen_range(0..360))
    }).collect();

    loop {
        if !window.is_open() || window.is_key_down(Key::Escape) { break; }

        // Handle resize
        let (new_w, new_h) = window.get_size();
        let new_w = new_w.max(400);
        let new_h = new_h.max(300);
        if new_w != win_w || new_h != win_h {
            win_w = new_w;
            win_h = new_h;
            buf.resize(win_w * win_h, 0);
        }
        let lay = Layout::compute(win_w, win_h);
        let ww = lay.win_w;
        let wh = lay.win_h;
        let cell = lay.cell;
        let board_x = lay.board_x;
        let board_y = lay.board_y;

        let keys = window.get_keys();
        let newly_pressed: Vec<Key> = keys.iter().filter(|k| !prev_keys.contains(k)).copied().collect();

        // Toggle auto-play
        for &k in &newly_pressed {
            if k == Key::T { game.auto_play = !game.auto_play; game.ai_target = None; }
        }

        if !game.game_over {
            if game.auto_play {
                if game.ai_target.is_none() {
                    game.ai_target = Some(game.ai_find_best());
                }
                game.ai_timer += 1.0 / 60.0;
                let ai_speed = 0.005;
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
            if !game.score_saved {
                save_score(&db, game.score, game.lines, game.level);
                cached_high_scores = get_top_scores(&db, 5);
                game.score_saved = true;
            }
            for &k in &newly_pressed {
                if k == Key::R { game = Game::new(); last_drop = Instant::now(); }
            }
        }

        // === DRAW ===
        buf.fill(NES_BG);
        frame += 1;

        // Stars (normalized positions)
        for (i, &(sx_f, sy_f, phase)) in stars.iter().enumerate() {
            let sx = (sx_f * ww as f64) as usize;
            let sy = (sy_f * wh as f64) as usize;
            let t = ((frame as f64 / 30.0) + phase as f64 / 60.0).sin();
            let brightness = ((t * 0.5 + 0.5) * 255.0) as u32;
            let star_c = (brightness << 16) | (brightness << 8) | brightness;
            set_pixel(&mut buf, ww, wh, sx, sy, star_c);
            if i % 3 == 0 {
                set_pixel(&mut buf, ww, wh, sx + 1, sy, star_c);
                set_pixel(&mut buf, ww, wh, sx, sy + 1, star_c);
            }
        }

        // Cathedral background - scale based on window height
        let ground_y = wh - (wh / 18);
        let scale_f = wh as f64 / DEFAULT_WIN_H as f64;
        let cat_x = ((ww as f64) * 0.22) as i32;

        // Horizon gradient
        let grad_h = (120.0 * scale_f) as usize;
        for y in (ground_y.saturating_sub(grad_h))..ground_y {
            let t = (y as i32 - (ground_y as i32 - grad_h as i32)).max(0) as u32;
            let r = t / 8; let g = t / 6; let b = (t / 3).min(40);
            let c = (r << 16) | (g << 8) | b;
            for x in 0..ww { set_pixel(&mut buf, ww, wh, x, y, c); }
        }

        // Ground
        fill_rect(&mut buf, ww, wh, 0, ground_y, ww, wh - ground_y, 0x1A1008);
        for i in 0..30 {
            let cx = (i * 23 + 7) % ww;
            let cy = ground_y + (i * 7 + 3) % (wh - ground_y).max(1);
            fill_rect(&mut buf, ww, wh, cx, cy, 8, 3, 0x221810);
        }
        fill_rect(&mut buf, ww, wh, 0, ground_y, ww, 2, 0x383028);

        // Helper: draw striped onion dome (scaled)
        let draw_onion = |buf: &mut [u32], cx: i32, base_y: i32, w: i32, h: i32,
                          c1: u32, c2: u32, stripe_w: i32, gold_tip: bool| {
            let cross_c = 0xD8B800;
            let tip_y = base_y - h - (18.0 * scale_f) as i32;
            fill_rect(buf, ww, wh, (cx - 1) as usize, tip_y as usize, 2, (14.0 * scale_f) as usize, cross_c);
            fill_rect(buf, ww, wh, (cx - 4) as usize, (tip_y + (4.0 * scale_f) as i32) as usize, 8, 2, cross_c);
            if gold_tip {
                fill_rect(buf, ww, wh, (cx - 2) as usize, (tip_y - 3) as usize, 5, 5, cross_c);
                fill_rect(buf, ww, wh, (cx - 1) as usize, (tip_y - 4) as usize, 3, 7, cross_c);
            }
            for dy in 0..h {
                let t = dy as f64 / h as f64;
                let width = if t < 0.15 {
                    (t / 0.15 * w as f64 * 0.2) as i32
                } else if t < 0.5 {
                    let bt = (t - 0.15) / 0.35;
                    (w as f64 * (0.2 + bt * 0.8)) as i32
                } else if t < 0.8 { w } else {
                    let bt = (t - 0.8) / 0.2;
                    (w as f64 * (1.0 - bt * 0.3)) as i32
                };
                let y = (base_y - h + dy) as usize;
                let x_start = cx - width / 2;
                for dx in 0..width {
                    let x = (x_start + dx) as usize;
                    let stripe = if stripe_w > 0 { ((dx + dy / 3) / stripe_w) % 2 == 0 } else { (dy / 3) % 2 == 0 };
                    let color = if stripe { c1 } else { c2 };
                    if x < ww && y < wh { set_pixel(buf, ww, wh, x, y, color); }
                }
            }
        };

        // Scaled cathedral drawing
        let s = scale_f;
        let brick = 0xB85838; let brick_dark = 0x883828; let brick_lite = 0xD07050;
        let white_trim = 0xE8E0D0; let green_roof = 0x286828;
        let ct_x = cat_x; let ct_base = ground_y as i32;
        let si = |v: i32| (v as f64 * s) as i32;
        let su = |v: usize| (v as f64 * s) as usize;

        // Main tower
        fill_rect(&mut buf, ww, wh, (ct_x + si(-35)) as usize, (ct_base + si(-60)) as usize, su(70), su(60), brick);
        fill_rect(&mut buf, ww, wh, (ct_x + si(-36)) as usize, (ct_base + si(-60)) as usize, su(72), su(3), white_trim);
        fill_rect(&mut buf, ww, wh, (ct_x + si(-36)) as usize, (ct_base + si(-30)) as usize, su(72), su(2), white_trim);
        for wx in [-20i32, -5, 10] {
            fill_rect(&mut buf, ww, wh, (ct_x + si(wx)) as usize, (ct_base + si(-52)) as usize, su(8), su(16), 0x181008);
            fill_rect(&mut buf, ww, wh, (ct_x + si(wx + 1)) as usize, (ct_base + si(-54)) as usize, su(6), su(2), brick_lite);
        }
        fill_rect(&mut buf, ww, wh, (ct_x + si(-22)) as usize, (ct_base + si(-110)) as usize, su(44), su(50), brick_lite);
        fill_rect(&mut buf, ww, wh, (ct_x + si(-24)) as usize, (ct_base + si(-110)) as usize, su(48), su(3), white_trim);
        fill_rect(&mut buf, ww, wh, (ct_x + si(-24)) as usize, (ct_base + si(-80)) as usize, su(48), su(2), white_trim);
        for kx in [-16i32, -4, 8] {
            fill_rect(&mut buf, ww, wh, (ct_x + si(kx)) as usize, (ct_base + si(-106)) as usize, su(10), su(2), 0xD8B800);
            fill_rect(&mut buf, ww, wh, (ct_x + si(kx + 2)) as usize, (ct_base + si(-108)) as usize, su(6), su(2), 0xD8B800);
        }
        for wx in [-14i32, 4] {
            fill_rect(&mut buf, ww, wh, (ct_x + si(wx)) as usize, (ct_base + si(-100)) as usize, su(6), su(12), 0x181008);
        }
        fill_rect(&mut buf, ww, wh, (ct_x + si(-14)) as usize, (ct_base + si(-155)) as usize, su(28), su(45), brick);
        fill_rect(&mut buf, ww, wh, (ct_x + si(-16)) as usize, (ct_base + si(-155)) as usize, su(32), su(2), white_trim);
        for i in 0..8 {
            let zx = ct_x + si(-12 + i * 3);
            fill_rect(&mut buf, ww, wh, zx as usize, (ct_base + si(-150)) as usize, su(2), su(2), 0xD8B800);
            fill_rect(&mut buf, ww, wh, (zx + si(1)) as usize, (ct_base + si(-148)) as usize, su(2), su(2), 0xD8B800);
        }
        for dy in 0..su(30) {
            let w = ((su(30) - dy) * su(14)) / su(30).max(1);
            let yp = ct_base + si(-185) + dy as i32;
            fill_rect(&mut buf, ww, wh, (ct_x - w as i32 / 2) as usize, yp as usize, w, 1, brick_lite);
            if dy % 4 < 2 {
                fill_rect(&mut buf, ww, wh, (ct_x - w as i32 / 2) as usize, yp as usize, w, 1, 0xD07050);
            }
        }
        draw_onion(&mut buf, ct_x, ct_base + si(-185), si(14), si(20), 0xD8B800, 0xC0A000, 0, true);

        // Left domes
        let ld1_x = ct_x + si(-60);
        fill_rect(&mut buf, ww, wh, (ld1_x + si(-16)) as usize, (ct_base + si(-75)) as usize, su(32), su(75), brick_dark);
        fill_rect(&mut buf, ww, wh, (ld1_x + si(-18)) as usize, (ct_base + si(-75)) as usize, su(36), su(3), white_trim);
        for wx in [-8i32, 4] { fill_rect(&mut buf, ww, wh, (ld1_x + si(wx)) as usize, (ct_base + si(-65)) as usize, su(6), su(10), 0x181008); }
        draw_onion(&mut buf, ld1_x, ct_base + si(-75), si(26), si(36), 0x286828, 0xC8B020, (4.0 * s) as i32, true);

        let ld2_x = ct_x + si(-25);
        fill_rect(&mut buf, ww, wh, (ld2_x + si(-14)) as usize, (ct_base + si(-90)) as usize, su(28), su(90), brick);
        fill_rect(&mut buf, ww, wh, (ld2_x + si(-16)) as usize, (ct_base + si(-90)) as usize, su(32), su(2), white_trim);
        fill_rect(&mut buf, ww, wh, (ld2_x + si(-16)) as usize, (ct_base + si(-60)) as usize, su(32), su(2), white_trim);
        for wx in [-8i32, 2] { fill_rect(&mut buf, ww, wh, (ld2_x + si(wx)) as usize, (ct_base + si(-82)) as usize, su(6), su(12), 0x181008); }
        draw_onion(&mut buf, ld2_x, ct_base + si(-90), si(22), si(30), 0x2050C8, 0xE8E8F0, (3.0 * s) as i32, true);

        // Right domes
        let rd1_x = ct_x + si(55);
        fill_rect(&mut buf, ww, wh, (rd1_x + si(-16)) as usize, (ct_base + si(-80)) as usize, su(32), su(80), brick);
        fill_rect(&mut buf, ww, wh, (rd1_x + si(-18)) as usize, (ct_base + si(-80)) as usize, su(36), su(3), white_trim);
        for wx in [-8i32, 4] { fill_rect(&mut buf, ww, wh, (rd1_x + si(wx)) as usize, (ct_base + si(-70)) as usize, su(6), su(10), 0x181008); }
        draw_onion(&mut buf, rd1_x, ct_base + si(-80), si(24), si(32), 0xC82020, 0x206828, (3.0 * s) as i32, true);

        let rd2_x = ct_x + si(28);
        fill_rect(&mut buf, ww, wh, (rd2_x + si(-12)) as usize, (ct_base + si(-85)) as usize, su(24), su(85), brick_lite);
        fill_rect(&mut buf, ww, wh, (rd2_x + si(-14)) as usize, (ct_base + si(-85)) as usize, su(28), su(2), white_trim);
        for wx in [-6i32, 2] { fill_rect(&mut buf, ww, wh, (rd2_x + si(wx)) as usize, (ct_base + si(-78)) as usize, su(5), su(10), 0x181008); }
        draw_onion(&mut buf, rd2_x, ct_base + si(-85), si(18), si(24), 0xC8A020, 0x388830, (3.0 * s) as i32, true);

        // Far towers
        let fl_x = ct_x + si(-90);
        fill_rect(&mut buf, ww, wh, (fl_x + si(-10)) as usize, (ct_base + si(-55)) as usize, su(20), su(55), brick_dark);
        fill_rect(&mut buf, ww, wh, (fl_x + si(-12)) as usize, (ct_base + si(-55)) as usize, su(24), su(2), white_trim);
        for dy in 0..su(20) {
            let w = ((su(20) - dy) * su(12)) / su(20).max(1);
            let c = if dy % 3 == 0 { 0x183018 } else { 0x284828 };
            fill_rect(&mut buf, ww, wh, (fl_x - w as i32 / 2) as usize, (ct_base + si(-75) + dy as i32) as usize, w, 1, c);
        }
        draw_onion(&mut buf, fl_x, ct_base + si(-75), si(10), si(14), 0x2030A0, 0x602060, (2.0 * s) as i32, true);

        let fr_x = ct_x + si(82);
        fill_rect(&mut buf, ww, wh, (fr_x + si(-10)) as usize, (ct_base + si(-50)) as usize, su(20), su(50), brick_dark);
        fill_rect(&mut buf, ww, wh, (fr_x + si(-12)) as usize, (ct_base + si(-50)) as usize, su(24), su(2), white_trim);
        for dy in 0..su(15) {
            let w = ((su(15) - dy) * su(14)) / su(15).max(1);
            fill_rect(&mut buf, ww, wh, (fr_x - w as i32 / 2) as usize, (ct_base + si(-65) + dy as i32) as usize, w, 1, green_roof);
        }

        // Base wall
        fill_rect(&mut buf, ww, wh, (ct_x + si(-95)) as usize, (ct_base + si(-18)) as usize, su(190), su(18), brick);
        fill_rect(&mut buf, ww, wh, (ct_x + si(-96)) as usize, (ct_base + si(-18)) as usize, su(192), su(2), white_trim);
        fill_rect(&mut buf, ww, wh, (ct_x + si(-96)) as usize, (ct_base - 1) as usize, su(192), su(2), white_trim);
        for ax in [-60i32, -20, 20, 50] {
            fill_rect(&mut buf, ww, wh, (ct_x + si(ax)) as usize, (ct_base + si(-16)) as usize, su(14), su(16), 0x100808);
            fill_rect(&mut buf, ww, wh, (ct_x + si(ax + 1)) as usize, (ct_base + si(-18)) as usize, su(12), su(2), brick_lite);
            fill_rect(&mut buf, ww, wh, (ct_x + si(ax + 2)) as usize, (ct_base + si(-20)) as usize, su(10), su(2), brick_lite);
            fill_rect(&mut buf, ww, wh, (ct_x + si(ax + 3)) as usize, (ct_base + si(-22)) as usize, su(8), su(2), brick_lite);
        }

        // Window glow
        for wx in [-60i32, -25, 28, 55] {
            let glow = ((frame as f64 / 40.0) + wx as f64 / 10.0).sin() * 0.3 + 0.7;
            let gi = (glow * 60.0) as u32;
            let gc = (gi << 16) | ((gi * 9 / 10) << 8) | (gi / 3);
            fill_rect(&mut buf, ww, wh, (ct_x + si(wx + 3)) as usize, (ct_base + si(-12)) as usize, su(8), su(8), gc);
        }

        // Snowflakes
        for i in 0..80 {
            let speed = 0.4 + (i as f64 * 0.618).fract() * 0.8;
            let drift = ((frame as f64 * 0.02 + i as f64 * 1.7).sin()) * 30.0 * s;
            let sx = ((i as f64 * 51.7 * s + drift) % ww as f64 + ww as f64) as usize % ww;
            let sy = ((frame as f64 * speed + i as f64 * 41.3) % (ground_y as f64 + 20.0)) as usize;
            let brightness = (((frame as f64 / 25.0 + i as f64).sin() * 0.2 + 0.8) * 255.0) as u32;
            let sc = (brightness << 16) | (brightness << 8) | brightness;
            if sy < wh {
                set_pixel(&mut buf, ww, wh, sx, sy, sc);
                if i % 3 == 0 && sx + 1 < ww {
                    set_pixel(&mut buf, ww, wh, sx + 1, sy, sc);
                    if sy + 1 < wh { set_pixel(&mut buf, ww, wh, sx, sy + 1, sc); }
                }
                if i % 7 == 0 && sx + 2 < ww && sy + 2 < wh {
                    set_pixel(&mut buf, ww, wh, sx + 1, sy + 1, sc);
                    set_pixel(&mut buf, ww, wh, sx + 2, sy, sc);
                    set_pixel(&mut buf, ww, wh, sx, sy + 2, sc);
                }
            }
        }
        // Snow accumulation
        for i in 0..ww {
            let h = (((i as f64 * 0.05).sin() * 2.0 + 3.0) as usize).min(6);
            fill_rect(&mut buf, ww, wh, i, ground_y.saturating_sub(h), 1, h, 0xD0D8E0);
        }

        // Title
        let title_x = board_x + BOARD_W * cell / 2 - 72;
        let title_y = board_y.saturating_sub(50).max(2);
        fill_rect(&mut buf, ww, wh, title_x.saturating_sub(15), title_y.saturating_sub(4), 180, 34, 0x0000A8);
        draw_rect_outline(&mut buf, ww, wh, title_x.saturating_sub(15), title_y.saturating_sub(4), 180, 34, 0xD8B800);
        draw_rect_outline(&mut buf, ww, wh, title_x.saturating_sub(13), title_y.saturating_sub(2), 176, 30, 0xA81010);
        fill_rect(&mut buf, ww, wh, title_x.saturating_sub(18), title_y.saturating_sub(1), 6, 6, 0xD8B800);
        fill_rect(&mut buf, ww, wh, title_x + 162, title_y.saturating_sub(1), 6, 6, 0xD8B800);
        draw_text(&mut buf, ww, wh, "TETRIS", title_x + 18, title_y + 4, 3, NES_TEXT);

        // Board border
        let bx = board_x.saturating_sub(6);
        let by = board_y.saturating_sub(6);
        let bw = BOARD_W * cell + 12;
        let bh = BOARD_H * cell + 12;
        fill_rect(&mut buf, ww, wh, bx, by, bw, bh, NES_PANEL_BG);
        draw_rect_outline(&mut buf, ww, wh, bx, by, bw, bh, 0xD8B800);
        draw_rect_outline(&mut buf, ww, wh, bx + 1, by + 1, bw - 2, bh - 2, 0xA81010);
        draw_rect_outline(&mut buf, ww, wh, bx + 2, by + 2, bw - 4, bh - 4, NES_BORDER);
        for &(cx, cy) in &[(bx, by), (bx + bw - 6, by), (bx, by + bh - 6), (bx + bw - 6, by + bh - 6)] {
            fill_rect(&mut buf, ww, wh, cx, cy, 6, 6, 0xD8B800);
        }

        // Board field
        fill_rect(&mut buf, ww, wh, board_x, board_y, BOARD_W * cell, BOARD_H * cell, NES_FIELD_BG);

        // Locked cells
        for r in 0..BOARD_H {
            for c in 0..BOARD_W {
                if let Some(piece_idx) = game.board[r][c] {
                    draw_nes_block(&mut buf, ww, wh, board_x + c * cell, board_y + r * cell, cell, piece_idx);
                }
            }
        }

        if !game.game_over {
            // Ghost piece
            let gr = game.ghost();
            let gc = game.cells(gr, game.col, game.rot);
            let pc = COLORS[game.piece];
            let ghost_pad = (cell / 12).max(1);
            for (r, c) in gc {
                if r >= 0 {
                    let ghost_c = blend(NES_FIELD_BG, pc, 30);
                    fill_rect(&mut buf, ww, wh,
                        (board_x as i32 + c * cell as i32 + ghost_pad as i32) as usize,
                        (board_y as i32 + r * cell as i32 + ghost_pad as i32) as usize,
                        cell - ghost_pad * 2, cell - ghost_pad * 2, ghost_c);
                }
            }

            // Current piece
            for (r, c) in game.cur() {
                if r >= 0 {
                    draw_nes_block(&mut buf, ww, wh,
                        (board_x as i32 + c * cell as i32) as usize,
                        (board_y as i32 + r * cell as i32) as usize, cell, game.piece);
                }
            }
        }

        // Right panel
        let px = board_x + BOARD_W * cell + 16;
        let pw = 170.min(ww.saturating_sub(px + 10));

        // Score + Lines box
        let mut py = board_y;
        fill_rect(&mut buf, ww, wh, px, py, pw, 68, NES_PANEL_BG);
        draw_rect_outline(&mut buf, ww, wh, px, py, pw, 68, 0xD8B800);
        draw_rect_outline(&mut buf, ww, wh, px + 1, py + 1, pw.saturating_sub(2), 66, NES_BORDER);
        draw_text(&mut buf, ww, wh, "SCORE", px + 8, py + 5, 2, 0xD8B800);
        draw_text(&mut buf, ww, wh, &format!("{:06}", game.score), px + 8, py + 22, 3, NES_SCORE_COLOR);
        draw_text(&mut buf, ww, wh, "LINES", px + 8, py + 48, 2, 0xD8B800);
        draw_text(&mut buf, ww, wh, &format!("{:03}", game.lines), px + 80, py + 48, 2, NES_SCORE_COLOR);

        // Next box
        py += 78;
        let next_cell = (cell * 5 / 6).max(8);
        fill_rect(&mut buf, ww, wh, px, py, pw, 82, NES_PANEL_BG);
        draw_rect_outline(&mut buf, ww, wh, px, py, pw, 82, 0xD8B800);
        draw_rect_outline(&mut buf, ww, wh, px + 1, py + 1, pw.saturating_sub(2), 80, NES_BORDER);
        draw_text(&mut buf, ww, wh, "NEXT", px + 50, py + 5, 2, 0xD8B800);
        fill_rect(&mut buf, ww, wh, px + 20, py + 20, pw.saturating_sub(40), 1, 0xD8B800);
        let no = TETROMINOES[game.next][0];
        for &(dr, dc) in &no {
            draw_nes_block(&mut buf, ww, wh, px + 45 + dc as usize * next_cell, py + 28 + dr as usize * next_cell, next_cell, game.next);
        }

        // Level box
        py += 92;
        fill_rect(&mut buf, ww, wh, px, py, pw, 42, NES_PANEL_BG);
        draw_rect_outline(&mut buf, ww, wh, px, py, pw, 42, 0xD8B800);
        draw_rect_outline(&mut buf, ww, wh, px + 1, py + 1, pw.saturating_sub(2), 40, NES_BORDER);
        draw_text(&mut buf, ww, wh, "LEVEL", px + 8, py + 5, 2, 0xD8B800);
        draw_text(&mut buf, ww, wh, &format!("{:02}", game.level), px + 80, py + 5, 3, NES_SCORE_COLOR);
        draw_text(&mut buf, ww, wh, "TYPE A", px + 8, py + 26, 2, 0x6888FC);

        // Controls box
        py += 52;
        fill_rect(&mut buf, ww, wh, px, py, pw, 82, NES_PANEL_BG);
        draw_rect_outline(&mut buf, ww, wh, px, py, pw, 82, 0xD8B800);
        draw_rect_outline(&mut buf, ww, wh, px + 1, py + 1, pw.saturating_sub(2), 80, NES_BORDER);
        draw_text(&mut buf, ww, wh, "CONTROLS", px + 30, py + 5, 2, 0xD8B800);
        fill_rect(&mut buf, ww, wh, px + 10, py + 4, 4, 4, 0xA81010);
        fill_rect(&mut buf, ww, wh, px + pw.saturating_sub(14), py + 4, 4, 4, 0xA81010);
        draw_text(&mut buf, ww, wh, "ARROWS/WASD", px + 8, py + 22, 2, 0x6888FC);
        draw_text(&mut buf, ww, wh, "SPACE: DROP", px + 8, py + 38, 2, 0x6888FC);
        draw_text(&mut buf, ww, wh, "R:NEW T:AI", px + 8, py + 54, 2, 0x6888FC);
        if game.auto_play {
            draw_text(&mut buf, ww, wh, "AI ON", px + 40, py + 68, 2, 0x00FF88);
        }

        // Tetris flash effect
        if game.tetris_flash > 0.0 {
            game.tetris_flash -= 1.0 / 60.0;
            let intensity = (game.tetris_flash * 3.0).min(1.0);
            for fpy in board_y..board_y + BOARD_H * cell {
                for fpx in board_x..board_x + BOARD_W * cell {
                    if fpy < wh && fpx < ww {
                        let idx = fpy * ww + fpx;
                        buf[idx] = blend(buf[idx], 0xFFFFFF, (intensity * 120.0) as u32);
                    }
                }
            }
            let t = (1.0 - game.tetris_flash) * 6.0;
            let rainbow = match (t as u32) % 6 {
                0 => 0xFF0000, 1 => 0xFF8800, 2 => 0xFFFF00,
                3 => 0x00FF00, 4 => 0x0088FF, _ => 0xFF00FF,
            };
            let cx = board_x + (BOARD_W * cell) / 2;
            let cy = board_y + (BOARD_H * cell) / 2;
            let tscale = 4;
            let tw = 7 * 6 * tscale;
            draw_text(&mut buf, ww, wh, "TETRIS!", cx - tw / 2, cy - tscale * 4, tscale, rainbow);
            let sparkle_w = (intensity * BOARD_W as f64 * cell as f64 * 0.5) as usize;
            let sparkle_y = cy + tscale * 5;
            if sparkle_w > 0 {
                fill_rect(&mut buf, ww, wh, cx - sparkle_w / 2, sparkle_y, sparkle_w, 3, rainbow);
                fill_rect(&mut buf, ww, wh, cx - sparkle_w / 2, sparkle_y.saturating_sub(20), sparkle_w, 3, rainbow);
            }
        }

        // Game over overlay with high scores
        if game.game_over {
            let cx = board_x + (BOARD_W * cell) / 2;
            let cy = board_y + (BOARD_H * cell) / 2;
            let panel_h = 90 + cached_high_scores.len() * 16 + if cached_high_scores.is_empty() { 0 } else { 24 };
            let top = cy.saturating_sub(panel_h / 2);
            fill_rect(&mut buf, ww, wh, cx.saturating_sub(110), top, 220, panel_h, NES_PANEL_BG);
            draw_rect_outline(&mut buf, ww, wh, cx.saturating_sub(110), top, 220, panel_h, NES_BORDER);
            draw_rect_outline(&mut buf, ww, wh, cx.saturating_sub(109), top + 1, 218, panel_h.saturating_sub(2), NES_BORDER);
            draw_text(&mut buf, ww, wh, "GAME OVER", cx.saturating_sub(80), top + 10, 3, 0xD82800);
            draw_text(&mut buf, ww, wh, "PRESS R", cx.saturating_sub(50), top + 40, 2, NES_TEXT);

            if !cached_high_scores.is_empty() {
                draw_text(&mut buf, ww, wh, "HIGH SCORES", cx.saturating_sub(66), top + 62, 2, 0xD8B800);
                for (i, hs) in cached_high_scores.iter().enumerate() {
                    let y = top + 80 + i * 16;
                    let entry = format!("{}.{:>6} L{:>2}", i + 1, hs.score, hs.level);
                    let color = if i == 0 { 0xD8B800 } else { NES_TEXT };
                    draw_text(&mut buf, ww, wh, &entry, cx.saturating_sub(90), y, 2, color);
                }
            }
        }

        window.update_with_buffer(&buf, ww, wh).unwrap();
        prev_keys = keys;
    }
}
