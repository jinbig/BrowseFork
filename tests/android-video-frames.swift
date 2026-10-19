import AVFoundation
import CoreImage
import Foundation

// Decode every recorded display frame, rather than sampling slow ADB screenshots.
let asset = AVURLAsset(url: URL(fileURLWithPath: CommandLine.arguments[1]))
let directory = URL(fileURLWithPath: CommandLine.arguments[2], isDirectory: true)
let track = asset.tracks(withMediaType: .video).first!
let reader = try AVAssetReader(asset: asset)
let output = AVAssetReaderTrackOutput(track: track, outputSettings: [
    kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA
])
reader.add(output)
reader.startReading()
let context = CIContext()
let colorSpace = CGColorSpaceCreateDeviceRGB()
let analysisWidth: CGFloat = 270
var frames: [[String: Any]] = []
while let sample = output.copyNextSampleBuffer() {
    let buffer = CMSampleBufferGetImageBuffer(sample)!
    let source = CIImage(cvPixelBuffer: buffer)
    let scale = analysisWidth / source.extent.width
    let image = source.transformed(by: CGAffineTransform(scaleX: scale, y: scale))
    let name = String(format: "%04d.png", frames.count)
    try context.writePNGRepresentation(of: image, to: directory.appendingPathComponent(name),
                                      format: .RGBA8, colorSpace: colorSpace)
    frames.append(["file": name, "ms": Int(CMSampleBufferGetPresentationTimeStamp(sample).seconds * 1000)])
}
guard reader.status == .completed else { fatalError("Video decoding failed: \(String(describing: reader.error))") }
try JSONSerialization.data(withJSONObject: frames).write(to: directory.appendingPathComponent("frames.json"))
