use owo_colors::Rgb;

pub struct ColorGenerator {
    index: usize,
}

impl ColorGenerator {
    pub fn new() -> ColorGenerator {
        ColorGenerator { index: 0 }
    }

    pub fn next(&mut self) -> Rgb {
        let color = COLORS[self.index];
        self.index = (self.index + 1) % COLORS.len();
        color
    }
}

// ‘Light’ colors from:
// https://personal.sron.nl/~pault/#sec:sequential
const COLORS: [Rgb; 8] = [
    Rgb(0x77, 0xAA, 0xDD), // light blue
    Rgb(0xEE, 0x88, 0x66), // orange
    Rgb(0xEE, 0xDD, 0x88), // light	 yellow
    Rgb(0x99, 0xDD, 0xFF), // light cyan
    Rgb(0xFF, 0xAA, 0xBB), // pink
    Rgb(0x44, 0xBB, 0x99), // mint
    Rgb(0xBB, 0xCC, 0x33), // pear
    Rgb(0xAA, 0xAA, 0x00), // olive
];
