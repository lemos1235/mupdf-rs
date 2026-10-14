use std::fmt::Write;

use mupdf::{Document, TextPage, TextPageFlags};

// A self-contained Type 3 font gives the glyphs explicit Unicode mappings and
// advances, without depending on system fonts or checking in a binary fixture.
fn text_page(content: &str, flags: TextPageFlags) -> TextPage {
    text_page_with_mark_advance(content, flags, 0)
}

fn text_page_with_mark_advance(content: &str, flags: TextPageFlags, mark_advance: u16) -> TextPage {
    fn stream(content: &str) -> String {
        format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            content.len(),
            content
        )
    }

    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 400] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_owned(),
        stream(content),
        format!("<< /Type /Font /Subtype /Type3 /FontBBox [-500 -200 1000 1000] /FontMatrix [0.001 0 0 0.001 0 0] /CharProcs << /base 7 0 R /mark 8 0 R /next 7 0 R /wide 9 0 R /arrow 8 0 R /overlay 8 0 R /secondmark 8 0 R >> /Encoding << /Type /Encoding /Differences [1 /base /mark /next /wide /arrow /overlay /secondmark] >> /FirstChar 1 /LastChar 7 /Widths [500 {mark_advance} 500 950 {mark_advance} {mark_advance} {mark_advance}] /Resources << >> /ToUnicode 6 0 R >>"),
        stream(
            "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
             /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
             /CMapName /Marks def /CMapType 2 def\n\
             1 begincodespacerange <00> <FF> endcodespacerange\n\
             7 beginbfchar <01> <0078> <02> <0302> <03> <0079> <04> <0057> <05> <20D7> <06> <0338> <07> <0301> endbfchar\n\
             endcmap CMapName currentdict /CMap defineresource pop end end",
        ),
        stream("500 0 0 0 500 700 d1 0 0 500 700 re f"),
        stream(&format!("{mark_advance} 0 -250 600 250 850 d1 -250 600 500 250 re f")),
        stream("950 0 0 0 950 700 d1 0 0 950 700 re f"),
    ];
    let mut pdf = "%PDF-1.7\n".to_owned();
    let mut offsets = vec![0];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        writeln!(pdf, "{} 0 obj\n{object}\nendobj", i + 1).unwrap();
    }
    let xref = pdf.len();
    writeln!(pdf, "xref\n0 {}\n0000000000 65535 f ", offsets.len()).unwrap();
    for offset in &offsets[1..] {
        writeln!(pdf, "{offset:010} 00000 n ").unwrap();
    }
    writeln!(
        pdf,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
        offsets.len()
    )
    .unwrap();

    let doc = Document::from_copied_bytes(pdf.as_bytes(), "pdf").unwrap();
    doc.load_page(0).unwrap().to_text_page(flags).unwrap()
}

fn lines(page: &TextPage) -> Vec<String> {
    let mut lines = Vec::new();
    for block in page.blocks() {
        for line in block.lines() {
            lines.push(line.chars().map(|ch| ch.char().unwrap()).collect());
        }
    }
    lines
}

#[test]
fn positioned_combining_mark_does_not_insert_a_space() {
    // Relative positions from HarfBuzz shaping Arial's "x\u{0302}y":
    // base advance = 0.5 em; mark offset = (-524, 25) / 2048 em.
    for flags in [TextPageFlags::empty(), TextPageFlags::ACCURATE_BBOXES] {
        let page = text_page(
            "BT /F1 20 Tf 1 0 0 1 50 300 Tm <01> Tj\n\
             1 0 0 1 54.8828125 300.244140625 Tm <02> Tj\n\
             1 0 0 1 60 300 Tm <03> Tj ET",
            flags,
        );
        assert_eq!(lines(&page), ["x\u{0302}y"]);
        assert_eq!(page.words().len(), 1);
        assert!(!page.search("x\u{0302}y").unwrap().is_empty());

        let block = page.blocks().next().unwrap();
        let line = block.lines().next().unwrap();
        let mark = line.chars().nth(1).unwrap();
        assert!((mark.origin().x - 54.882_813).abs() < 0.001);
    }
}

#[test]
fn combining_marks_on_a_wide_base_do_not_break_lines() {
    let page = text_page(
        "BT /F1 20 Tf 1 0 0 1 50 300 Tm <04> Tj\n\
         1 0 0 1 50 300 Tm <02> Tj <07> Tj\n\
         1 0 0 1 69 300 Tm <03> Tj ET",
        TextPageFlags::INHIBIT_SPACES,
    );
    assert_eq!(lines(&page), ["W\u{0302}\u{0301}y"]);
}

#[test]
fn independently_positioned_marks_do_not_attach_to_the_previous_line() {
    for mark in ["02", "05", "06"] {
        let page = text_page(
            &format!(
                "BT /F1 20 Tf 1 0 0 1 50 300 Tm <0103> Tj\n\
                 1 0 0 1 50 250 Tm <{mark}> Tj <01> Tj ET"
            ),
            TextPageFlags::empty(),
        );
        let expected_mark = match mark {
            "02" => '\u{0302}',
            "05" => '\u{20d7}',
            _ => '\u{0338}',
        };
        assert_eq!(lines(&page), ["xy".to_owned(), format!("{expected_mark}x")]);
    }
}

#[test]
fn independently_positioned_mark_does_not_attach_to_a_distant_word() {
    let page = text_page(
        "BT /F1 20 Tf 1 0 0 1 50 300 Tm <0103> Tj\n\
         1 0 0 1 150 300 Tm <05> Tj <01> Tj ET",
        TextPageFlags::empty(),
    );
    assert_eq!(lines(&page), ["xy", "\u{20d7}x"]);
}

#[test]
fn rotated_combining_mark_does_not_insert_a_space() {
    let page = text_page(
        "BT /F1 20 Tf 0 1 -1 0 50 250 Tm <01> Tj\n\
         0 1 -1 0 49.755859375 254.8828125 Tm <02> Tj\n\
         0 1 -1 0 50 260 Tm <03> Tj ET",
        TextPageFlags::empty(),
    );
    assert_eq!(lines(&page), ["x\u{0302}y"]);
}

#[test]
fn mark_with_a_different_rotation_starts_its_own_line() {
    let page = text_page(
        "BT /F1 20 Tf 1 0 0 1 50 300 Tm <0103> Tj\n\
         0 1 -1 0 70 300 Tm <02> Tj <01> Tj ET",
        TextPageFlags::empty(),
    );
    assert_eq!(lines(&page), ["xy", "\u{0302}x"]);
}

#[test]
fn nonzero_advance_mark_preserves_its_encoded_spacing() {
    // A PDF may map a spacing glyph to a non-spacing Unicode character.
    let page = text_page_with_mark_advance(
        "BT /F1 20 Tf 1 0 0 1 50 300 Tm <010203> Tj ET",
        TextPageFlags::empty(),
        500,
    );
    assert_eq!(lines(&page), ["x\u{0302}y"]);
}

#[test]
fn repeated_mark_at_the_same_position_is_not_extracted_twice() {
    let page = text_page(
        "BT /F1 20 Tf 1 0 0 1 50 300 Tm <01020203> Tj ET",
        TextPageFlags::empty(),
    );
    assert_eq!(lines(&page), ["x\u{0302}y"]);
}

#[test]
fn preserve_spans_separates_a_mark_with_a_different_font_size() {
    let page = text_page(
        "BT /F1 20 Tf 1 0 0 1 50 300 Tm <01> Tj\n\
         /F1 10 Tf 1 0 0 1 55 300 Tm <02> Tj ET",
        TextPageFlags::PRESERVE_SPANS,
    );
    assert_eq!(lines(&page), ["x", "\u{0302}"]);
}

#[test]
fn actualtext_combining_suffix_does_not_insert_a_space() {
    let page = text_page(
        "/Span << /ActualText <FEFF00780302> >> BDC\n\
         BT /F1 20 Tf 1 0 0 1 50 300 Tm <01> Tj ET EMC\n\
         BT /F1 20 Tf 1 0 0 1 60 300 Tm <03> Tj ET",
        TextPageFlags::empty(),
    );
    assert_eq!(lines(&page), ["x\u{0302}y"]);
}
