use std::cell::RefCell;

use array2d::Array2D;
mod array2d;
use rand::prelude::*;

#[link(wasm_import_module = "kernel")]
extern "C" {
    fn kernel(ptr: *mut f32, width: f32, height: f32, x: f32, y: f32, time: f32);
}

pub fn call_kernel(width: f32, height: f32, x: f32, y: f32, time: f32) -> [f32; 4] {
    let mut out_data = [0_f32; 4];

    unsafe {
        kernel(out_data.as_mut_ptr(), width, height, x, y, time);
    }

    out_data
}

#[no_mangle]
pub extern "C" fn make_image(width: u32, height: u32, time: f32) -> *const f32 {
    thread_local! {
        static BUFFER: RefCell<Option<Plugin>> = RefCell::new(None);
    }

    BUFFER.with(|buffer| {
        let mut maybe_plugin = buffer.borrow_mut();
        let plugin = maybe_plugin.get_or_insert_with(|| Plugin::new(width, height));

        plugin.get_image(time).as_ptr()
    })
}

struct Plugin {
    output_buf: Array2D<[f32; 4]>,
    sim: Sim,
}

impl Plugin {
    pub fn new(out_width: u32, out_height: u32) -> Self {
        Self {
            output_buf: Array2D::new(out_width as _, out_height as _),
            sim: Sim::new(out_width as _, out_height as _),
        }
    }

    pub fn get_image(&mut self, time: f32) -> &[f32] {
        self.sim.step(time);
        self.output_buf = self.sim.draw();

        bytemuck::cast_slice(self.output_buf.data())
    }
}

struct Sim {
    cells: Array2D<f32>,
    coords: Array2D<(usize, usize)>,
    rng: SmallRng,
}

impl Sim {
    pub fn new(w: usize, h: usize) -> Self {
        let mut cells = Array2D::new(w, h);

        for y in 0..cells.height() {
            for x in 0..cells.width() {
                let [sx, sy] = [x, y].map(|v| v as f32);

                let rgba = call_kernel(cells.width() as f32, cells.height() as f32, sx, sy, 0.);

                cells[(x, y)] = rgba[0];
            }
        }

        let coords = coord_array(&cells);
        Self {
            coords,
            cells,
            rng: SmallRng::seed_from_u64(0xDEAD_BEEF_C0DE_D1CC),
        }
    }

    pub fn step(&mut self, _time: f32) {
        self.coords.data_mut().shuffle(&mut self.rng);
        let (w, h) = (self.coords.width(), self.coords.height());

        let next_frame = self
            .coords
            .data()
            .iter()
            .zip(all_coords(w, h))
            .map(|(&scramble, origin)| {
                let (x1, y1) = origin;
                let (x2, y2) = scramble;

                let (dx, dy) = (x1 as f32 - x2 as f32, y1 as f32 - y2 as f32);
                let dist = (dx.powi(2) + dy.powi(2)).sqrt();
                let dist = dist / 5000.;

                let k = 1. / (dist + 1.);
                self.cells[origin] * k + self.cells[scramble] * (1. - k)
            })
            .collect();
        self.cells = Array2D::from_array(self.coords.width(), next_frame);
    }

    pub fn draw(&self) -> Array2D<[f32; 4]> {
        map_array(&self.cells, |&val| [val, val, val, 1.])
    }
}

fn map_array<T, U>(arr: &Array2D<T>, f: impl FnMut(&T) -> U) -> Array2D<U> {
    Array2D::from_array(arr.width(), arr.data().iter().map(f).collect())
}

fn coord_array<T>(arr: &Array2D<T>) -> Array2D<(usize, usize)> {
    let data = all_coords(arr.width(), arr.height()).collect();
    Array2D::from_array(arr.width(), data)
}

fn all_coords(w: usize, h: usize) -> impl Iterator<Item = (usize, usize)> {
    (0..h).map(move |y| (0..w).map(move |x| (x, y))).flatten()
}
