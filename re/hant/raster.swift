import Foundation
import CoreText
import CoreGraphics
// usage: raster FONT SIZE CELL CHARS_FILE OUT_BIN  -> OUT_BIN = n * cell*cell bytes (0/1)
let a = CommandLine.arguments
let url = URL(fileURLWithPath: a[1]) as CFURL
let size = CGFloat(Double(a[2])!)
let cell = Int(a[3])!
let chars = Array(try! String(contentsOfFile: a[4], encoding: .utf8))
let descs = CTFontManagerCreateFontDescriptorsFromURL(url) as! [CTFontDescriptor]
let font = CTFontCreateWithFontDescriptor(descs[0], size, nil)
let ascent = CTFontGetAscent(font), descent = CTFontGetDescent(font)
var out = Data()
var missing = 0
for ch in chars {
    var bytes = [UInt8](repeating: 0, count: cell * cell)
    let ctx = CGContext(data: &bytes, width: cell, height: cell, bitsPerComponent: 8, bytesPerRow: cell,
                        space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue)!
    ctx.setAllowsAntialiasing(false); ctx.setShouldAntialias(false)
    ctx.setShouldSmoothFonts(false)
    ctx.setFillColor(gray: 1, alpha: 1)
    let attr = [kCTFontAttributeName: font, kCTForegroundColorFromContextAttributeName: true] as CFDictionary
    let s = CFAttributedStringCreate(nil, String(ch) as CFString, attr)!
    let line = CTLineCreateWithAttributedString(s)
    let w = CTLineGetTypographicBounds(line, nil, nil, nil)
    // centre the em box in the cell
    let x = (CGFloat(cell) - CGFloat(w)) / 2
    let y = (CGFloat(cell) - (ascent + descent)) / 2 + descent
    ctx.textPosition = CGPoint(x: x.rounded(), y: y.rounded())
    CTLineDraw(line, ctx)
    var glyphs = [CGGlyph](repeating: 0, count: 2); var u = Array(String(ch).utf16)
    if !CTFontGetGlyphsForCharacters(font, &u, &glyphs, u.count) { missing += 1 }
    out.append(contentsOf: bytes.map { $0 > 127 ? 1 : 0 })
}
try! out.write(to: URL(fileURLWithPath: a[5]))
FileHandle.standardError.write("glyphs \(chars.count) missing \(missing) ascent \(ascent) descent \(descent)\n".data(using: .utf8)!)
