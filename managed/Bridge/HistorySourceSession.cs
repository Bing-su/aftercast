using Aftercast.ActCompat;

namespace Aftercast.Bridge;

internal sealed record HistorySourceRequest(ulong Owner = 0, HistorySelection? Selection = null,
    HistorySourceQuery? Query = null, string? Path = null, ImportTextOptions? Text = null,
    ImportCompanion[]? Companions = null, long? MaximumBytes = null)
{
    internal void Validate(int command)
    {
        if (command == 3)
        {
            if (Owner != 0 || Selection is null || Query is null || Path is not null || Text is not null || Companions is not null || MaximumBytes is not null)
                throw new ArgumentException("Invalid source proposal request");
        }
        else if (command == 6)
        {
            if (Owner == 0 || Selection is not null || Query is not null || string.IsNullOrWhiteSpace(Path) ||
                Companions is null || MaximumBytes is <= 0 or > 4294967296)
                throw new ArgumentException("Invalid source capture request");
            // Share explicit source/codec admission rules; range comes exclusively from the selected history.
            new CompanionRequest(Path: Path, Text: Text, Companions: Companions).Validate(1);
        }
        else if (Owner == 0 || Selection is not null || Query is not null || Path is not null || Text is not null || Companions is not null || MaximumBytes is not null)
            throw new ArgumentException("Invalid source continuation request");
    }
}

internal sealed partial class PluginSession
{
    private ulong proposalOwner, captureOwner;
    private Task<HistorySourceSnapshot>? proposal;
    private Task<HistorySourceCapture>? capture;
    private ImportCompanion[] captureSelections = [];
    private CancellationTokenSource? proposalCancellation, captureCancellation;

    internal byte[] HistorySourceOperation(int command, HistorySourceRequest request) => OnOwner(() =>
    {
        var host = Host();
        if (command == 3)
        {
            if (proposal is not null) throw new InvalidOperationException("Release the previous source proposal first");
            var cancellation = new CancellationTokenSource();
            try { proposal = host.ReadHistorySourcesAsync(request.Selection!, request.Query!, cancellation: cancellation.Token); }
            catch { cancellation.Dispose(); throw; }
            proposalCancellation = cancellation;
            proposalOwner = NewSourceOwner();
            return HostJson.Encode(new { Owner = proposalOwner, Pending = true });
        }
        if (command is 4 or 5 or 6)
        {
            if (request.Owner == 0 || request.Owner != proposalOwner || proposal is null)
                throw new InvalidOperationException("Source proposal owner is stale");
            if (command == 5)
            {
                if (capture is not null) throw new InvalidOperationException("Release the captured source first");
                ReleaseProposal();
                return HostJson.Encode(true);
            }
            if (!proposal.IsCompleted)
            {
                if (command == 6) throw new InvalidOperationException("Source proposal is still pending");
                return HostJson.Encode(new { Owner = proposalOwner, Pending = true });
            }
            var snapshot = proposal.GetAwaiter().GetResult();
            if (command == 4) return HostJson.Encode(new { Owner = proposalOwner, Pending = false, Snapshot = snapshot });
            if (capture is not null) throw new InvalidOperationException("Release the previous captured source first");
            var cancellation = new CancellationTokenSource();
            try { capture = host.CaptureHistorySourceAsync(snapshot, request.Path!, request.Text, request.Companions,
                request.MaximumBytes ?? 4L * 1024 * 1024 * 1024, cancellation.Token); }
            catch { cancellation.Dispose(); throw; }
            captureSelections = request.Companions!.ToArray();
            captureCancellation = cancellation;
            captureOwner = NewSourceOwner();
            return HostJson.Encode(new { Owner = captureOwner, Pending = true });
        }
        if (request.Owner == 0 || request.Owner != captureOwner || capture is null)
            throw new InvalidOperationException("Captured source owner is stale");
        if (command == 8)
        {
            ReleaseCapture();
            return HostJson.Encode(true);
        }
        if (!capture.IsCompleted)
        {
            if (command == 9) throw new InvalidOperationException("Source capture is still pending");
            return HostJson.Encode(new { Owner = captureOwner, Pending = true, Progress = host.HistoryCaptureProgress });
        }
        var image = capture.GetAwaiter().GetResult();
        if (command == 7) return HostJson.Encode(new { Owner = captureOwner, Pending = false, Files = image.Files.Select(file => new { file.Path, file.Name, file.Length, file.Sha256, file.Text,
                SelectedName = file.Name.Length == 0 ? "" : captureSelections.Single(selected =>
                    string.Equals(selected.Name, file.Name, StringComparison.OrdinalIgnoreCase)).Name }),
            StartTicks = image.Range.Start.Ticks, EndTicks = image.Range.End.Ticks });
        RequireSourceParser();
        var importOwner = NewSourceOwner();
        // Keep a Ready capture on preflight failure, but invalidate any transferred image even if admission throws.
        try { host.BeginCapturedHistoryImport(image); }
        finally { if (image.State != HistorySourceCaptureState.Ready) ForgetCapture(); }
        sourceImportOwner = importOwner;
        return HostJson.Encode(new SourceImportReply(importOwner, true));
    });
    private void ForgetCapture()
    {
        capture = null; captureOwner = 0; captureSelections = [];
        captureCancellation?.Dispose(); captureCancellation = null;
    }
    private void ReleaseCapture()
    {
        captureCancellation?.Cancel();
        try { capture?.GetAwaiter().GetResult().Dispose(); }
        catch (OperationCanceledException) { }
        // A failed operation remains observable through poll; release itself always relinquishes ownership.
        finally { ForgetCapture(); }
    }
    private void ReleaseProposal()
    {
        proposalCancellation?.Cancel();
        try { proposal?.GetAwaiter().GetResult(); }
        catch (OperationCanceledException) { }
        finally { proposal = null; proposalOwner = 0; proposalCancellation?.Dispose(); proposalCancellation = null; }
    }
}
