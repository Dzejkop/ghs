use ratatui::{buffer::Buffer, layout::Rect};

pub fn blit(tgt: &mut Buffer, src: &Buffer, area: Rect, src_offset: (u16, u16)) {
    let (src_offset_x, src_offset_y) = src_offset;

    let src_area = src.area;

    // Iterate over target buffer area
    for tgt_y in area.y..(area.y + area.height) {
        for tgt_x in area.x..(area.x + area.width) {
            // Convert target position to relative coordinates
            let rel_x = tgt_x - area.x;
            let rel_y = tgt_y - area.y;

            // // Offset target coordinates
            // let tgt_x = tgt_x + tgt_offset_x;
            // let tgt_y = tgt_y + tgt_offset_y;

            // Calculate source position with offset
            let src_x = src_area.x + rel_x + src_offset_x;
            let src_y = src_area.y + rel_y + src_offset_y;

            // Only copy if source position is valid
            copy_cel(tgt, src, tgt_x, tgt_y, src_x, src_y);
        }
    }
}

#[inline(always)]
fn copy_cel(tgt: &mut Buffer, src: &Buffer, tgt_x: u16, tgt_y: u16, src_x: u16, src_y: u16) {
    let Some(src_cell) = src.cell((src_x, src_y)) else {
        return;
    };
    let Some(tgt_cell) = tgt.cell_mut((tgt_x, tgt_y)) else {
        return;
    };
    tgt_cell.set_symbol(src_cell.symbol());
    tgt_cell.set_style(src_cell.style());
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{layout::Rect, style::Style};

    fn display_buffer(buf: &Buffer) -> String {
        let mut s = String::new();

        let w = (buf.area.width - buf.area.x) as usize;
        let h = (buf.area.height - buf.area.y) as usize;

        for row in 0..h {
            for col in 0..w {
                let Some(c) = buf.cell((row as u16, col as u16)) else {
                    continue;
                };

                s += c.symbol();
            }
            s += "\n";
        }

        s
    }

    #[test]
    fn whatever() {
        let buf = Buffer::empty(Rect::new(0, 0, 5, 5));

        let disp = display_buffer(&buf);

        assert_eq!(disp, "     \n     \n     \n     \n     \n");
    }

    #[test]
    fn display_test() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 5, 5));
        buf.set_string(1, 1, "x", Style::default());

        let disp = display_buffer(&buf);

        assert_eq!(disp, "     \n x   \n     \n     \n     \n");
    }

    #[test]
    fn basic_blit() {
        let mut src_buf = Buffer::empty(Rect::new(0, 0, 5, 5));
        src_buf.set_string(3, 3, "x", Style::default());

        let mut tgt_buf = Buffer::empty(Rect::new(0, 0, 5, 5));
        tgt_buf.set_string(4, 4, "x", Style::default());

        blit(&mut tgt_buf, &src_buf, Rect::new(0, 0, 5, 5), (0, 0));

        let disp = display_buffer(&tgt_buf);

        assert_eq!(disp, "     \n     \n     \n   x \n     \n");
    }

    #[test]
    fn blit_smaller_with_offset() {
        let mut src_buf = Buffer::empty(Rect::new(0, 0, 2, 2));
        src_buf.set_string(0, 0, "x", Style::default());

        let mut tgt_buf = Buffer::empty(Rect::new(0, 0, 5, 5));
        tgt_buf.set_string(4, 4, "y", Style::default());

        blit(&mut tgt_buf, &src_buf, Rect::new(1, 1, 2, 2), (0, 0));

        let disp = display_buffer(&tgt_buf);

        assert_eq!(disp, "     \n x   \n     \n     \n    y\n");
    }

    #[test]
    fn blit_larger_with_offset() {
        let mut src_buf = Buffer::empty(Rect::new(0, 0, 5, 5));
        src_buf.set_string(0, 0, "12345", Style::default());
        src_buf.set_string(0, 1, "12345", Style::default());
        src_buf.set_string(0, 2, "12345", Style::default());
        src_buf.set_string(0, 3, "12345", Style::default());
        src_buf.set_string(0, 4, "12345", Style::default());

        let mut tgt_buf = Buffer::empty(Rect::new(0, 0, 2, 2));

        blit(&mut tgt_buf, &src_buf, Rect::new(0, 0, 5, 5), (2, 2));

        let disp = display_buffer(&tgt_buf);

        assert_eq!(disp, "33\n44\n");
    }
}
