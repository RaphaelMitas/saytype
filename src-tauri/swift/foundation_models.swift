import Foundation
import FoundationModels

@_cdecl("saytype_fm_availability")
public func saytypeFmAvailability() -> Int32 {
    guard #available(macOS 26.0, *) else { return 4 }
    switch SystemLanguageModel.default.availability {
    case .available: return 0
    case .unavailable(.deviceNotEligible): return 1
    case .unavailable(.appleIntelligenceNotEnabled): return 2
    case .unavailable(.modelNotReady): return 3
    case .unavailable: return 1
    }
}

/// Blocks the calling thread. Returns a malloc'd string the caller frees with
/// `saytype_fm_free`: the rewrite when `ok` is set to 1, otherwise an error message.
@_cdecl("saytype_fm_respond")
public func saytypeFmRespond(
    _ instructions: UnsafePointer<CChar>,
    _ text: UnsafePointer<CChar>,
    _ timeoutMs: Int64,
    _ ok: UnsafeMutablePointer<Int32>
) -> UnsafeMutablePointer<CChar>? {
    ok.pointee = 0
    guard #available(macOS 26.0, *) else {
        return strdup("Apple Intelligence needs macOS 26 or later")
    }

    let instructions = String(cString: instructions)
    let text = String(cString: text)

    final class Box: @unchecked Sendable {
        var result: Result<String, Error>?
    }
    let box = Box()
    let done = DispatchSemaphore(value: 0)
    let task = Task.detached(priority: .userInitiated) {
        defer { done.signal() }
        do {
            box.result = .success(try await rewrite(instructions: instructions, text: text))
        } catch {
            box.result = .failure(error)
        }
    }

    if done.wait(timeout: .now() + .milliseconds(Int(timeoutMs))) == .timedOut {
        task.cancel()
        return strdup("Timed out after \(timeoutMs) ms")
    }
    switch box.result! {
    case .success(let rewritten):
        ok.pointee = 1
        return strdup(rewritten)
    case .failure(let error):
        return strdup(error.localizedDescription)
    }
}

@_cdecl("saytype_fm_free")
public func saytypeFmFree(_ pointer: UnsafeMutablePointer<CChar>?) {
    free(pointer)
}

@available(macOS 26.0, *)
private func rewrite(instructions: String, text: String) async throws -> String {
    // Dictation is the user's own text, so the default guardrails refuse too much of it
    let model = SystemLanguageModel(guardrails: .permissiveContentTransformations)
    let session = LanguageModelSession(model: model, instructions: instructions)
    let response = try await session.respond(to: text, options: GenerationOptions(temperature: 0))
    return response.content
}
