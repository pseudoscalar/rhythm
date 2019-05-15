use specs::prelude::*;

pub struct ClearColor(pub Color);

impl Default for ClearColor {
    fn default() -> ClearColor { ClearColor(Color::rgb(0,0,0)) }
}

#[derive(Debug)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Component for Position {
    type Storage = VecStorage<Self>;
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Copy)]
pub struct Rectangle {
    pub width: f64,
    pub height: f64,
}

impl Component for Rectangle {
    type Storage = VecStorage<Self>;
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color {r, g, b}
    }
}

impl Component for Color {
    type Storage = VecStorage<Self>;
}


