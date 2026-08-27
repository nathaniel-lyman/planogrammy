import AppKit
import Foundation

struct Scene {
    let name: String
    let background: String
    let eyebrow: String
    let headline: String
    let caption: String
    let meta: String
    let prompt: String?
    let metricCard: Bool
    let final: Bool
}

let scriptURL = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL
let reelDirectory = scriptURL.deletingLastPathComponent()
let frameDirectory = reelDirectory.appendingPathComponent("frames", isDirectory: true)
try FileManager.default.createDirectory(at: frameDirectory, withIntermediateDirectories: true)

let scenes = [
    Scene(name: "01-title", background: "01-catalog-and-empty-mod.png", eyebrow: "LOCAL PRODUCT DEMO", headline: "Merchandising intent becomes exact, reviewable shelf changes.", caption: "Planogrammy combines a Rust/Wasm domain engine, a WebGPU canvas, and live WebMCP agent tools.", meta: "DETERMINISTIC  •  VALIDATED  •  REVISION-SAFE", prompt: nil, metricCard: false, final: false),
    Scene(name: "02-fields", background: "07-tray-performance-fields.png", eyebrow: "NEW CATALOG INTELLIGENCE", headline: "Every metric stays exact and explainable.", caption: "Depth, net weight, SSW, USW, margin, casepack, and loaded tray geometry are visible together.", meta: "TRAILING 13-WEEK SYNTHETIC DEMO DATA  •  NOT RETAILER ACTUALS", prompt: nil, metricCard: true, final: false),
    Scene(name: "03-prompt1", background: "01-catalog-and-empty-mod.png", eyebrow: "MERCHANDISING BRIEF", headline: "Ask for the outcome, not pixel coordinates.", caption: "The agent reads the open draft, catalog, fixture, and physical constraints.", meta: "", prompt: "Fill this 4-foot mod with all 22 SKUs in contiguous brand blocks.", metricCard: false, final: false),
    Scene(name: "04-proposal1", background: "02-brand-block-proposal.png", eyebrow: "NON-MUTATING PROPOSAL", headline: "22 additions  •  6 brand blocks  •  all constraints pass", caption: "Rust resolves tray footprints, exact coordinates, and the 1/8-inch minimum gap before anything changes.", meta: "REVISION 0  •  PREVIEW ONLY", prompt: nil, metricCard: false, final: false),
    Scene(name: "05-approve1", background: "03-brand-block-approved.png", eyebrow: "HUMAN APPROVAL BOUNDARY", headline: "One approval records one atomic change set.", caption: "The mod is filled only after the user accepts the reviewed proposal.", meta: "REVISION 0 → 1  •  CHANGE_0001", prompt: nil, metricCard: false, final: false),
    Scene(name: "06-prompt2", background: "03-brand-block-approved.png", eyebrow: "PERFORMANCE BRIEF", headline: "The agent can reason from the new performance fields.", caption: "Brand blocking remains a constraint while the assortment changes.", meta: "", prompt: "Remove low performing items and spread the strongest sellers.", metricCard: false, final: false),
    Scene(name: "07-proposal2", background: "04-performance-proposal.png", eyebrow: "PERFORMANCE-LED PROPOSAL", headline: "7 stronger additions  •  7 removals  •  5 deterministic moves", caption: "The lowest sales-per-store-per-week items are replaced without breaking the approved brand blocks.", meta: "REVISION 1  •  PREVIEW ONLY  •  ALL CONSTRAINTS PASS", prompt: nil, metricCard: false, final: false),
    Scene(name: "08-capacity", background: "08-capacity-fill-proposal.png", eyebrow: "CAPACITY-LED EXPANSION", headline: "23 more high-performing placements  •  92–100% utilization", caption: "The agent fills remaining physical capacity without breaking brand blocks, tray footprints, or the 1/8-inch gap.", meta: "REVISION 2  •  PREVIEW ONLY  •  ALL CONSTRAINTS PASS", prompt: nil, metricCard: false, final: false),
    Scene(name: "09-dense-approved", background: "09-capacity-approved.png", eyebrow: "APPROVED DENSE MOD", headline: "45 placements use the full four-foot mod.", caption: "Each shelf reaches 92–100% utilization with the strongest sellers expanded inside their brand blocks.", meta: "REVISION 2 → 3  •  CHANGE_0003", prompt: nil, metricCard: false, final: false),
    Scene(name: "10-final", background: "10-dense-final.png", eyebrow: "FINAL LOCAL RESULT", headline: "45 placements  •  revision 7  •  all changes local", caption: "A dense, physically valid, performance-led mod that remains reviewable and reversible.", meta: "NO LIVE RETAILER RECORDS TOUCHED", prompt: nil, metricCard: false, final: true),
]

let canvasSize = NSSize(width: 1280, height: 720)
let white = NSColor(calibratedWhite: 0.98, alpha: 1)
let muted = NSColor(calibratedRed: 0.84, green: 0.88, blue: 0.91, alpha: 1)
let blue = NSColor(calibratedRed: 0.34, green: 0.58, blue: 1, alpha: 1)
let green = NSColor(calibratedRed: 0.54, green: 0.88, blue: 0.66, alpha: 1)

func fillRounded(_ rect: NSRect, radius: CGFloat, color: NSColor, border: NSColor? = nil) {
    let path = NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius)
    color.setFill()
    path.fill()
    if let border {
        border.setStroke()
        path.lineWidth = 1
        path.stroke()
    }
}

func drawText(_ text: String, in rect: NSRect, font: NSFont, color: NSColor, lineHeight: CGFloat? = nil) {
    let paragraph = NSMutableParagraphStyle()
    paragraph.lineBreakMode = .byWordWrapping
    if let lineHeight {
        paragraph.minimumLineHeight = lineHeight
        paragraph.maximumLineHeight = lineHeight
    }
    NSAttributedString(string: text, attributes: [
        .font: font,
        .foregroundColor: color,
        .paragraphStyle: paragraph,
    ]).draw(with: rect, options: [.usesLineFragmentOrigin, .usesFontLeading])
}

func drawPill(_ text: String) {
    let font = NSFont.systemFont(ofSize: 12, weight: .heavy)
    let measured = (text as NSString).size(withAttributes: [.font: font])
    let rect = NSRect(x: 30, y: 26, width: measured.width + 46, height: 34)
    fillRounded(rect, radius: 17, color: NSColor(calibratedRed: 0.055, green: 0.078, blue: 0.102, alpha: 0.9), border: NSColor(calibratedWhite: 1, alpha: 0.22))
    blue.setFill()
    NSBezierPath(ovalIn: NSRect(x: 43, y: 39, width: 8, height: 8)).fill()
    drawText(text, in: NSRect(x: 61, y: 36, width: measured.width + 4, height: 18), font: font, color: white)
}

func drawCopy(_ scene: Scene) {
    let card = NSRect(x: 34, y: 532, width: 1212, height: 158)
    fillRounded(card, radius: 14, color: NSColor(calibratedRed: 0.05, green: 0.071, blue: 0.09, alpha: 0.91), border: NSColor(calibratedWhite: 1, alpha: 0.15))
    drawText(scene.headline, in: NSRect(x: 60, y: 552, width: 1138, height: 43), font: NSFont.systemFont(ofSize: 34, weight: .bold), color: white, lineHeight: 37)
    drawText(scene.caption, in: NSRect(x: 60, y: 601, width: 1138, height: 29), font: NSFont.systemFont(ofSize: 18, weight: .medium), color: muted, lineHeight: 23)
    if !scene.meta.isEmpty {
        drawText(scene.meta, in: NSRect(x: 60, y: 649, width: 1138, height: 20), font: NSFont.systemFont(ofSize: 13, weight: .heavy), color: scene.final ? green : blue)
    }
}

func drawPrompt(_ prompt: String) {
    NSColor(calibratedRed: 0.035, green: 0.052, blue: 0.07, alpha: 0.64).setFill()
    NSRect(origin: .zero, size: canvasSize).fill()
    let card = NSRect(x: 315, y: 170, width: 650, height: 270)
    fillRounded(card, radius: 18, color: NSColor(calibratedWhite: 0.985, alpha: 0.98), border: NSColor(calibratedWhite: 1, alpha: 0.35))
    fillRounded(NSRect(x: 349, y: 202, width: 28, height: 28), radius: 8, color: NSColor(calibratedRed: 0.095, green: 0.36, blue: 0.85, alpha: 1))
    drawText("U", in: NSRect(x: 358, y: 207, width: 12, height: 17), font: NSFont.systemFont(ofSize: 13, weight: .heavy), color: white)
    drawText("USER REQUEST", in: NSRect(x: 389, y: 207, width: 180, height: 18), font: NSFont.systemFont(ofSize: 12, weight: .heavy), color: NSColor(calibratedRed: 0.22, green: 0.36, blue: 0.62, alpha: 1))
    drawText(prompt, in: NSRect(x: 349, y: 250, width: 582, height: 118), font: NSFont.systemFont(ofSize: 31, weight: .bold), color: NSColor(calibratedRed: 0.08, green: 0.105, blue: 0.13, alpha: 1), lineHeight: 38)
    drawText("The agent works through Planogrammy's live semantic tools.", in: NSRect(x: 349, y: 394, width: 580, height: 22), font: NSFont.systemFont(ofSize: 14, weight: .medium), color: NSColor(calibratedWhite: 0.39, alpha: 1))
}

func drawMetricCard() {
    let card = NSRect(x: 325, y: 106, width: 610, height: 225)
    fillRounded(card, radius: 14, color: NSColor(calibratedRed: 0.05, green: 0.075, blue: 0.10, alpha: 0.93), border: NSColor(calibratedWhite: 1, alpha: 0.18))
    drawText("Performance + logistics, in one catalog view", in: NSRect(x: 353, y: 134, width: 555, height: 44), font: NSFont.systemFont(ofSize: 30, weight: .bold), color: white)
    let chips = ["Depth", "Net weight", "SSW", "USW", "Gross margin", "Casepack", "Loaded tray footprint"]
    var x: CGFloat = 353
    var y: CGFloat = 203
    for chip in chips {
        let font = NSFont.systemFont(ofSize: 14, weight: .semibold)
        let width = (chip as NSString).size(withAttributes: [.font: font]).width + 24
        if x + width > 907 { x = 353; y += 48 }
        fillRounded(NSRect(x: x, y: y, width: width, height: 34), radius: 17, color: NSColor(calibratedRed: 0.15, green: 0.35, blue: 0.72, alpha: 0.24), border: NSColor(calibratedRed: 0.57, green: 0.72, blue: 1, alpha: 0.4))
        drawText(chip, in: NSRect(x: x + 12, y: y + 8, width: width - 24, height: 18), font: font, color: NSColor(calibratedRed: 0.91, green: 0.95, blue: 1, alpha: 1))
        x += width + 9
    }
}

for scene in scenes {
    guard let background = NSImage(contentsOf: reelDirectory.appendingPathComponent(scene.background)) else {
        fatalError("Missing background: \(scene.background)")
    }
    let image = NSImage(size: canvasSize, flipped: true) { rect in
        background.draw(in: rect, from: .zero, operation: .copy, fraction: 1, respectFlipped: true, hints: nil)
        if scene.prompt == nil {
            let gradient = NSGradient(colorsAndLocations:
                (NSColor.clear, 0.40),
                (NSColor(calibratedRed: 0.035, green: 0.052, blue: 0.07, alpha: 0.82), 1.0)
            )!
            gradient.draw(in: rect, angle: -90)
        }
        if let prompt = scene.prompt { drawPrompt(prompt) }
        if scene.metricCard { drawMetricCard() }
        drawPill(scene.eyebrow)
        drawCopy(scene)
        return true
    }
    guard let tiff = image.tiffRepresentation,
          let bitmap = NSBitmapImageRep(data: tiff),
          let png = bitmap.representation(using: .png, properties: [:]) else {
        fatalError("Could not encode frame: \(scene.name)")
    }
    try png.write(to: frameDirectory.appendingPathComponent("\(scene.name).png"))
}

print("Rendered \(scenes.count) frames to \(frameDirectory.path)")
