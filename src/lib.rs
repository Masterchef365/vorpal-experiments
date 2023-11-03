use std::cell::RefCell;

use array2d::Array2D;
mod array2d;
use rand::prelude::*;

use fluid::{FluidSim, SmokeSim};

mod fluid;

#[link(wasm_import_module = "kernel")]
extern "C" {
    fn kernel(
        ptr: *mut f32,
        width: f32,
        height: f32,
        x: f32,
        y: f32,
        time: f32,
        cursor_x: f32,
        cursor_y: f32,
    );
}

pub fn call_kernel(
    width: f32,
    height: f32,
    x: f32,
    y: f32,
    time: f32,
    cursor_x: f32,
    cursor_y: f32,
) -> [f32; 4] {
    let mut out_data = [0_f32; 4];

    unsafe {
        kernel(
            out_data.as_mut_ptr(),
            width,
            height,
            x,
            y,
            time,
            cursor_x,
            cursor_y,
        );
    }

    out_data
}

#[no_mangle]
pub extern "C" fn make_image(
    width: u32,
    height: u32,
    time: f32,
    cursor_x: f32,
    cursor_y: f32,
) -> *const f32 {
    thread_local! {
        static BUFFER: RefCell<Option<Plugin>> = RefCell::new(None);
    }

    BUFFER.with(|buffer| {
        let mut maybe_plugin = buffer.borrow_mut();
        let plugin = maybe_plugin.get_or_insert_with(|| Plugin::new(width, height));

        plugin.get_image(time, cursor_x, cursor_y).as_ptr()
    })
}

struct Plugin {
    out_rgba: Vec<f32>,
    out_width: u32,
    out_height: u32,

    smoke_sim: SmokeSim,
    fluid_sim: FluidSim,

    last_cursor_x: f32,
    last_cursor_y: f32,
}

impl Plugin {
    pub fn new(out_width: u32, out_height: u32) -> Self {
        assert_eq!(out_width, out_height);
        let w = out_width as usize;
        let fluid_sim = FluidSim::new(w, w);
        let mut smoke_sim = SmokeSim::new(w, w);

        let intensity = 1e4;
        smoke_sim.smoke_mut()[(w / 2, w / 3)] = intensity;

        Self {
            out_rgba: vec![0_f32; (out_width * out_height * 4) as usize],
            out_width,
            out_height,

            fluid_sim,
            smoke_sim,

            last_cursor_x: -1.0,
            last_cursor_y: -1.0,
        }
    }

    pub fn get_image(&mut self, time: f32, cursor_x: f32, cursor_y: f32) -> &[f32] {
        // Draw smoke and push fluid
        /*
        let d = self.smoke_sim.smoke_mut();
        let center = (d.width() / 2, d.height() / 2);

        let (u, v) = self.fluid_sim.uv_mut();

        let pos = center;
        //let time = 3. * PI / 2.;
        u[pos] = -450. * (time).cos();
        v[pos] = -450. * (time).sin();
        */

        // Move fluid and smoke
        let dt = 1e-2;
        let overstep = 1.9;

        self.fluid_sim.step(dt, overstep, 15);
        self.smoke_sim.advect(self.fluid_sim.uv(), dt);

        self.out_rgba = self
            .smoke_sim
            .smoke()
            .data()
            .iter()
            .map(|v| [*v; 4])
            .flatten()
            .collect();

        // Make image
        if self.last_cursor_x != -1.0 && cursor_x != -1.0 {
            for y in 0..self.out_height {
                for x in 0..self.out_width {
                    let rgba = call_kernel(
                        self.out_width as f32,
                        self.out_height as f32,
                        x as f32,
                        y as f32,
                        time,
                        cursor_x,
                        cursor_y,
                    );

                    let delta_x = cursor_x - self.last_cursor_x;
                    let delta_y = cursor_y - self.last_cursor_y;

                    let (u, v) = self.fluid_sim.uv_mut();
                    let pos = (x as usize, y as usize);
                    u[pos] += rgba[0] * delta_x;
                    v[pos] += rgba[0] * delta_y;
                }
            }
        }

        /*
        if self.last_cursor_x != -1.0 && cursor_x != -1.0 {
            let delta_x = cursor_x - self.last_cursor_x;
            let delta_y = cursor_y - self.last_cursor_y;

            let pos = (cursor_x as usize, cursor_y as usize);
            u[pos] += delta_x;
            v[pos] += delta_y;
        }
        */

        self.last_cursor_x = cursor_x;
        self.last_cursor_y = cursor_y;

        &self.out_rgba
    }
}
