pub type Array2D = crate::array2d::Array2D<f32>;

use glam::{Vec2, IVec2};

pub struct SmokeSim {
    read: Array2D,
    write: Array2D,
}

#[derive(Clone)]
pub struct FluidState {
    u: Array2D,
    v: Array2D,
}

pub struct FluidSim {
    read: FluidState,
    write: FluidState,
}

impl FluidSim {
    pub fn new(width: usize, height: usize) -> Self {
        let empty = FluidState {
            u: Array2D::new(width + 1, height),
            v: Array2D::new(width, height + 1),
        };

        Self {
            read: empty.clone(),
            write: empty,
        }
    }

    pub fn step(&mut self, dt: f32, overstep: f32, n_iters: u32) {
        // Force incompressibility
        for _ in 0..n_iters {
            // Enforce boundary conditions
            for arr in [&mut self.read.v, &mut self.read.u] {
                let (w, h) = (arr.width(), arr.height());
                for y in 0..arr.height() {
                    arr[(1, y)] = 0.;
                    arr[(w-2, y)] = 0.;
                }

                for x in 0..arr.width() {
                    arr[(x, 1)] = 0.;
                    arr[(x, h-2)] = 0.;
                }
            }

            // Solve
            for y in 1..self.read.v.height() - 2 {
                for x in 1..self.read.u.width() - 2 {
                    let dx = self.read.u[(x + 1, y)] - self.read.u[(x, y)];
                    let dy = self.read.v[(x, y + 1)] - self.read.v[(x, y)];

                    let d = overstep * (dx + dy) / 4.;

                    self.read.u[(x, y)] += d;
                    self.read.u[(x + 1, y)] -= d;

                    self.read.v[(x, y)] += d;
                    self.read.v[(x, y + 1)] -= d;
                }
            }
        }

        // Advect velocity (u component)
        for y in 1..self.read.u.height() - 1 {
            for x in 1..self.read.u.width() - 1 {
                let (px, py) = advect(&self.read.u, &self.read.v, x as f32, y as f32 + 0.5, dt);
                self.write.u[(x, y)] = interp(&self.read.u, px, py - 0.5);
            }
        }

        // Advect velocity (v component)
        for y in 1..self.read.v.height() - 1 {
            for x in 1..self.read.v.width() - 1 {
                let (px, py) = advect(&self.read.u, &self.read.v, x as f32 + 0.5, y as f32, dt);
                self.write.v[(x, y)] = interp(&self.read.v, px - 0.5, py);
            }
        }

        // Swap the written buffers back into read again
        std::mem::swap(&mut self.read.u, &mut self.write.u);
        std::mem::swap(&mut self.read.v, &mut self.write.v);
    }

    pub fn uv(&self) -> (&Array2D, &Array2D) {
        (&self.read.u, &self.read.v)
    }

    pub fn uv_mut(&mut self) -> (&mut Array2D, &mut Array2D) {
        (&mut self.read.u, &mut self.read.v)
    }

    pub fn width(&self) -> usize {
        self.read.v.width()
    }

    pub fn height(&self) -> usize {
        self.read.u.height()
    }
}

impl SmokeSim {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            read: Array2D::new(width, height),
            write: Array2D::new(width, height),
        }
    }

    pub fn advect(&mut self, (u, v): (&Array2D, &Array2D), dt: f32) {
        // Advect smoke
        for y in 1..v.height() - 2 {
            for x in 1..v.width() - 2 {
                let (px, py) = advect(&u, &v, x as f32 + 0.5, y as f32 + 0.5, dt);
                self.write[(x, y)] = interp(&self.read, px - 0.5, py - 0.5);
            }
        }

        std::mem::swap(&mut self.read, &mut self.write);
    }

    pub fn smoke(&self) -> &Array2D {
        &self.read
    }

    pub fn smoke_mut(&mut self) -> &mut Array2D {
        &mut self.read
    }
}

/// Transport x and y (relative to fluid grid coordinates) along `u` and `v` by a step `dt`
fn advect(u: &Array2D, v: &Array2D, x: f32, y: f32, dt: f32) -> (f32, f32) {
    let u = interp(&u, x, y - 0.5);
    let v = interp(&v, x - 0.5, y);

    //let [u, v, _, _] = call_kernel(u, v, x, y, dt);

    let px = x - u * dt;
    let py = y - v * dt;

    (px, py)
}

/// Linear interpolation
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    (1. - t) * a + t * b
}

/// Bilinear interpolation of the given grid at the given coordinates
#[track_caller]
fn interp(grid: &Array2D, x: f32, y: f32) -> f32 {
    if x * 2.0 > grid.width() as f32 {
        cubic_interp(grid, Vec2::new(x, y))
    } else {
        bilinear_interp(grid, x, y)
    }
}

fn bilinear_interp(grid: &Array2D, x: f32, y: f32) -> f32 {
    // Bounds enforcement. No panics!
    let tl_x = (x.floor() as isize).clamp(0, grid.width() as isize - 1) as usize;
    let tl_y = (y.floor() as isize).clamp(0, grid.height() as isize - 1) as usize;

    // Get corners
    let tl = grid[(tl_x, tl_y)];
    let tr = grid[(tl_x + 1, tl_y)];
    let bl = grid[(tl_x, tl_y + 1)];
    let br = grid[(tl_x + 1, tl_y + 1)];

    // Bilinear interpolation
    lerp(
        lerp(tl, tr, x.fract()), // Top row
        lerp(bl, br, x.fract()), // Bottom row
        y.fract(),
    )
}

fn cubic_interp(img: &Array2D, pt: Vec2) -> f32 {
    let coeffs: [f32; 16] = [
        1. / 6., -1. / 2., 1. / 2., -1. / 6.,
        2. / 3., 0., -1., 1. / 2.,
        1. / 6., 1. / 2., 1. / 2., -1. / 2.,
        0., 0., 0., 1. / 6.
    ];

    let fr = pt.fract();
    let coord = pt.floor().as_ivec2();
    let mut col = 0.0;

    for i in 0..4 {
        for j in 0..4 {
            let mut t = Vec2::new(1.0, 1.0);
            let mut b = Vec2::ZERO;
            for k in 0..4 {
                b += t * Vec2::new(coeffs[i * 4 + k], coeffs[j * 4 + k]);
                t *= fr;
            }
            let x_idx = (coord.x + i as i32 - 1).clamp(0, img.width() as i32 - 1) as usize;
            let y_idx = (coord.y + j as i32 - 1).clamp(0, img.height() as i32 - 1) as usize;
            let smp = img[(x_idx, y_idx)];
            col += smp * b.x * b.y;
        }
    }

    col
}


/*
/// Bicubic interpolation of the given grid at the given coordinates
fn bicubic_interp(grid: &Array2D, x: f32, y: f32) -> f32 {
    // Bounds enforcement. No panics!
    let x_index = (x.floor() as isize).clamp(2, grid.width() as isize - 3);
    let y_index = (y.floor() as isize).clamp(2, grid.height() as isize - 3);

    // Calculate fractional parts of coordinates
    let dx = x - x.floor();
    let dy = y - y.floor();

    // Compute bicubic interpolation using 4x4 grid of neighboring points
    let mut interpolated_value = 0.0;
    for i in -1..=2isize {
        for j in -1..=2isize {
            let weight_x = bicubic_weight(dx - i as f32);
            let weight_y = bicubic_weight(dy - j as f32);
            let value = grid[((x_index + i) as usize, (y_index + j) as usize)];
            interpolated_value += value * weight_x * weight_y;
        }
    }

    interpolated_value
}

/// Bicubic interpolation weight function
fn bicubic_weight(t: f32) -> f32 {
    let t_abs = t.abs();
    if t_abs <= 1.0 {
        return 1.0 - (2.0 * t_abs.powi(2)) + (t_abs.powi(3));
    } else if t_abs <= 2.0 {
        return 4.0 - (8.0 * t_abs) + (5.0 * t_abs.powi(2)) - (t_abs.powi(3));
    }
    0.0
}
*/
