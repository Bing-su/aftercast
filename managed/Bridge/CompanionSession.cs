using Aftercast.ActCompat;

namespace Aftercast.Bridge;

internal sealed partial class PluginSession
{
    private ulong nextSourceOwner, sourceImportOwner, liveSourceOwner;
    private long liveRootGeneration;
    private CompanionRequest? liveRequest;
    private ulong NewSourceOwner() => nextSourceOwner = checked(nextSourceOwner + 1);
    private void RequireSourceParser()
    {
        if (!plugins.Values.Any(plugin => plugin.State == PluginState.Initialized)) throw new InvalidOperationException("Source operation requires an initialized plugin");
    }
    internal byte[] CompanionOperation(int command, CompanionRequest request) => OnOwner(() =>
    {
        if (command == 7) return HostJson.Encode(new { Path = request.Path!, Candidates = HostEnvironment.ReadCompanionCandidates(request.Path!) });
        if (command == 1)
        {
            RequireSourceParser();
            var owner = NewSourceOwner();
            Host().BeginImport(request.Path!, request.Range, request.Text, request.Companions, request.Viewpoint);
            sourceImportOwner = owner;
            return HostJson.Encode(new SourceImportReply(owner, true));
        }
        if (command is 2 or 3)
        {
            if (sourceImportOwner == 0 || sourceImportOwner != request.Owner) throw new InvalidOperationException("Source import owner is stale");
            try
            {
                var result = Host().AdvanceImport(command == 3);
                if (result is null) return HostJson.Encode(new SourceImportReply(request.Owner, true, Progress: Host().ImportProgress));
                sourceImportOwner = 0;
                return HostJson.Encode(new SourceImportReply(request.Owner, Result: result));
            }
            catch (OperationCanceledException) { sourceImportOwner = 0; return HostJson.Encode(new SourceImportReply(request.Owner, Cancelled: true)); }
            catch { sourceImportOwner = 0; throw; }
        }
        ReconcileLiveOwner();
        if (command == 4)
        {
            RequireSourceParser();
            if (request.Owner != liveSourceOwner) throw new InvalidOperationException("Live selection owner is stale");
            var host = Host(); EnsureCanStop();
            var owner = NewSourceOwner();
            var previous = host.LiveLogGeneration;
            liveSourceOwner = 0; liveRequest = null;
            liveRootGeneration = host.OpenLiveLog(request.Path!, request.Companions!, request.GetCurrentZone ?? false,
                TimeSpan.FromMilliseconds(request.ClockWaitMilliseconds ?? 30000), request.Text);
            if (liveRootGeneration != previous + 1) throw new InvalidOperationException("Live selection was replaced during open");
            liveSourceOwner = owner;
            liveRequest = request with { Companions = request.Companions!.ToArray() };
            return EncodeLive(owner);
        }
        RequireLiveOwner(request.Owner);
        if (command == 5) return EncodeLive(request.Owner);
        EnsureCanStop();
        Host().CloseLiveLog(liveRootGeneration);
        liveSourceOwner = 0; liveRequest = null;
        return HostJson.Encode(true);
    });
    private void ReconcileLiveOwner()
    {
        if (liveSourceOwner != 0 && (environment is null || environment.LiveLogGeneration != liveRootGeneration))
        { liveSourceOwner = 0; liveRequest = null; }
    }
    private void RequireLiveOwner(ulong owner)
    {
        ReconcileLiveOwner();
        if (owner == 0 || owner != liveSourceOwner) throw new InvalidOperationException("Live selection owner is stale");
    }
    private byte[] EncodeLive(ulong owner)
    {
        RequireLiveOwner(owner);
        var request = liveRequest!;
        var sources = Host().ReadLiveLogSources().Select(source =>
        {
            var length = Math.Min(source.LastError.Length, 2048);
            if (length != 0 && char.IsHighSurrogate(source.LastError[length - 1])) length--;
            // Keep request identity exact while the catalog retains physical filename casing.
            var selectedName = source.Name.Length == 0 ? "" : request.Companions!.Single(selected =>
                string.Equals(selected.Name, source.Name, StringComparison.OrdinalIgnoreCase)).Name;
            return new LiveSourceReply(selectedName, source.Name, source.Path, source.State, source.Queued, source.PendingTime?.Ticks,
                source.Generation, source.Dropped, source.LastError[..length], length != source.LastError.Length);
        }).ToArray();
        RequireLiveOwner(owner);
        return HostJson.Encode(new LiveSelectionReply(owner, request.Path!, request.Text, request.Companions!, request.GetCurrentZone ?? false,
            request.ClockWaitMilliseconds ?? 30000, sources));
    }
}
