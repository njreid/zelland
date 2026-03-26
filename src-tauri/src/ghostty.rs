use std::ffi::{c_void};
use std::ptr;

// Include the generated bindings
#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(non_upper_case_globals)]
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/ghostty_vt_bindings.rs"));
}

pub use bindings::*;

pub struct GhosttyTerminalWrapper {
    pub(crate) inner: GhosttyTerminal,
}

impl GhosttyTerminalWrapper {
    pub fn new(cols: u16, rows: u16) -> Result<Self, i32> {
        let mut terminal: GhosttyTerminal = ptr::null_mut();
        let options = GhosttyTerminalOptions {
            cols,
            rows,
            max_scrollback: 0, // We want zero local scrollback as per plan
        };

        let result = unsafe { ghostty_terminal_new(ptr::null(), &mut terminal, options) };

        if result == GhosttyResult_GHOSTTY_SUCCESS {
            Ok(Self { inner: terminal })
        } else {
            Err(result)
        }
    }

    pub fn write(&self, data: &[u8]) {
        unsafe {
            ghostty_terminal_vt_write(self.inner, data.as_ptr(), data.len());
        }
    }

    pub fn resize(&self, cols: u16, rows: u16, width_px: u32, height_px: u32) -> Result<(), i32> {
        let result = unsafe {
            ghostty_terminal_resize(
                self.inner,
                cols,
                rows,
                width_px,
                height_px,
            )
        };

        if result == GhosttyResult_GHOSTTY_SUCCESS {
            Ok(())
        } else {
            Err(result)
        }
    }

    pub fn get_size(&self) -> (u16, u16) {
        let mut cols: u16 = 0;
        let mut rows: u16 = 0;
        unsafe {
            ghostty_terminal_get(
                self.inner,
                GhosttyTerminalData_GHOSTTY_TERMINAL_DATA_COLS,
                &mut cols as *mut _ as *mut c_void,
            );
            ghostty_terminal_get(
                self.inner,
                GhosttyTerminalData_GHOSTTY_TERMINAL_DATA_ROWS,
                &mut rows as *mut _ as *mut c_void,
            );
        }
        (cols, rows)
    }

    pub fn get_cursor_pos(&self) -> (u16, u16) {
        let mut x: u16 = 0;
        let mut y: u16 = 0;
        unsafe {
            ghostty_terminal_get(
                self.inner,
                GhosttyTerminalData_GHOSTTY_TERMINAL_DATA_CURSOR_X,
                &mut x as *mut _ as *mut c_void,
            );
            ghostty_terminal_get(
                self.inner,
                GhosttyTerminalData_GHOSTTY_TERMINAL_DATA_CURSOR_Y,
                &mut y as *mut _ as *mut c_void,
            );
        }
        (x, y)
    }

    pub fn get_mouse_tracking(&self) -> bool {
        let mut tracking: bool = false;
        unsafe {
            ghostty_terminal_get(
                self.inner,
                GhosttyTerminalData_GHOSTTY_TERMINAL_DATA_MOUSE_TRACKING,
                &mut tracking as *mut _ as *mut c_void,
            );
        }
        tracking
    }
}

impl Drop for GhosttyTerminalWrapper {
    fn drop(&mut self) {
        unsafe {
            ghostty_terminal_free(self.inner);
        }
    }
}

unsafe impl Send for GhosttyTerminalWrapper {}
unsafe impl Sync for GhosttyTerminalWrapper {}

pub struct GhosttyRenderStateWrapper {
    inner: GhosttyRenderState,
    row_iter: GhosttyRenderStateRowIterator,
    row_cells: GhosttyRenderStateRowCells,
}

impl GhosttyRenderStateWrapper {
    pub fn new() -> Result<Self, i32> {
        let mut state: GhosttyRenderState = ptr::null_mut();
        let mut row_iter: GhosttyRenderStateRowIterator = ptr::null_mut();
        let mut row_cells: GhosttyRenderStateRowCells = ptr::null_mut();

        unsafe {
            let res = ghostty_render_state_new(ptr::null(), &mut state);
            if res != GhosttyResult_GHOSTTY_SUCCESS { return Err(res); }
            
            let res = ghostty_render_state_row_iterator_new(ptr::null(), &mut row_iter);
            if res != GhosttyResult_GHOSTTY_SUCCESS { 
                ghostty_render_state_free(state);
                return Err(res); 
            }

            let res = ghostty_render_state_row_cells_new(ptr::null(), &mut row_cells);
            if res != GhosttyResult_GHOSTTY_SUCCESS {
                ghostty_render_state_row_iterator_free(row_iter);
                ghostty_render_state_free(state);
                return Err(res);
            }
        }

        Ok(Self { 
            inner: state,
            row_iter,
            row_cells,
        })
    }

    pub fn update(&self, terminal: &GhosttyTerminalWrapper) -> Result<(), i32> {
        let result = unsafe { ghostty_render_state_update(self.inner, terminal.inner) };

        if result == GhosttyResult_GHOSTTY_SUCCESS {
            Ok(())
        } else {
            Err(result)
        }
    }

    pub fn get_dirty(&self) -> GhosttyRenderStateDirty {
        let mut dirty: GhosttyRenderStateDirty = 0;
        unsafe {
            ghostty_render_state_get(
                self.inner,
                GhosttyRenderStateData_GHOSTTY_RENDER_STATE_DATA_DIRTY,
                &mut dirty as *mut _ as *mut c_void,
            );
        }
        dirty
    }

    pub fn reset_dirty(&self) {
        let dirty = GhosttyRenderStateDirty_GHOSTTY_RENDER_STATE_DIRTY_FALSE;
        unsafe {
            ghostty_render_state_set(
                self.inner,
                GhosttyRenderStateOption_GHOSTTY_RENDER_STATE_OPTION_DIRTY,
                &dirty as *const _ as *const c_void,
            );
        }
    }

    pub fn get_size(&self) -> (u16, u16) {
        let mut cols: u16 = 0;
        let mut rows: u16 = 0;
        unsafe {
            ghostty_render_state_get(
                self.inner,
                GhosttyRenderStateData_GHOSTTY_RENDER_STATE_DATA_COLS,
                &mut cols as *mut _ as *mut c_void,
            );
            ghostty_render_state_get(
                self.inner,
                GhosttyRenderStateData_GHOSTTY_RENDER_STATE_DATA_ROWS,
                &mut rows as *mut _ as *mut c_void,
            );
        }
        (cols, rows)
    }

    /// Iterator over rows and cells for rendering.
    pub fn with_rows<F>(&mut self, mut f: F)
    where
        F: FnMut(u16, bool, &GhosttyRenderStateRowCells),
    {
        unsafe {
            ghostty_render_state_get(
                self.inner,
                GhosttyRenderStateData_GHOSTTY_RENDER_STATE_DATA_ROW_ITERATOR,
                &mut self.row_iter as *mut _ as *mut c_void,
            );

            let mut row_idx = 0;
            while ghostty_render_state_row_iterator_next(self.row_iter) {
                ghostty_render_state_row_get(
                    self.row_iter,
                    GhosttyRenderStateRowData_GHOSTTY_RENDER_STATE_ROW_DATA_CELLS,
                    &mut self.row_cells as *mut _ as *mut c_void,
                );

                let mut dirty: bool = false;
                ghostty_render_state_row_get(
                    self.row_iter,
                    GhosttyRenderStateRowData_GHOSTTY_RENDER_STATE_ROW_DATA_DIRTY,
                    &mut dirty as *mut _ as *mut std::ffi::c_void,
                );

                f(row_idx, dirty, &self.row_cells);

                if dirty {
                    let reset: bool = false;
                    ghostty_render_state_row_set(
                        self.row_iter,
                        GhosttyRenderStateRowOption_GHOSTTY_RENDER_STATE_ROW_OPTION_DIRTY,
                        &reset as *const _ as *const std::ffi::c_void,
                    );
                }

                row_idx += 1;
            }
        }
    }

    pub fn is_row_dirty(&self) -> bool {
        let mut dirty: bool = false;
        unsafe {
            ghostty_render_state_row_get(
                self.row_iter,
                GhosttyRenderStateRowData_GHOSTTY_RENDER_STATE_ROW_DATA_DIRTY,
                &mut dirty as *mut _ as *mut std::ffi::c_void,
            );
        }
        dirty
    }

    pub fn reset_row_dirty(&self) {
        let dirty: bool = false;
        unsafe {
            ghostty_render_state_row_set(
                self.row_iter,
                GhosttyRenderStateRowOption_GHOSTTY_RENDER_STATE_ROW_OPTION_DIRTY,
                &dirty as *const _ as *const std::ffi::c_void,
            );
        }
    }
}

impl Drop for GhosttyRenderStateWrapper {
    fn drop(&mut self) {
        unsafe {
            ghostty_render_state_row_cells_free(self.row_cells);
            ghostty_render_state_row_iterator_free(self.row_iter);
            ghostty_render_state_free(self.inner);
        }
    }
}

unsafe impl Send for GhosttyRenderStateWrapper {}
unsafe impl Sync for GhosttyRenderStateWrapper {}

pub fn get_cell_raw(cells: GhosttyRenderStateRowCells) -> GhosttyCell {
    let mut cell: GhosttyCell = 0;
    unsafe {
        ghostty_render_state_row_cells_get(
            cells,
            GhosttyRenderStateRowCellsData_GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_RAW,
            &mut cell as *mut _ as *mut c_void,
        );
    }
    cell
}

pub fn get_cell_style(cells: GhosttyRenderStateRowCells) -> GhosttyStyle {
    let mut style: GhosttyStyle = unsafe { std::mem::zeroed() };
    style.size = std::mem::size_of::<GhosttyStyle>();
    unsafe {
        ghostty_render_state_row_cells_get(
            cells,
            GhosttyRenderStateRowCellsData_GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_STYLE,
            &mut style as *mut _ as *mut c_void,
        );
    }
    style
}

pub fn get_cell_graphemes(cells: GhosttyRenderStateRowCells) -> Vec<u32> {
    let mut len: u32 = 0;
    unsafe {
        ghostty_render_state_row_cells_get(
            cells,
            GhosttyRenderStateRowCellsData_GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_LEN,
            &mut len as *mut _ as *mut c_void,
        );
    }

    if len == 0 { return Vec::new(); }

    let mut buf = vec![0u32; len as usize];
    unsafe {
        ghostty_render_state_row_cells_get(
            cells,
            GhosttyRenderStateRowCellsData_GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_BUF,
            buf.as_mut_ptr() as *mut c_void,
        );
    }
    buf
}
