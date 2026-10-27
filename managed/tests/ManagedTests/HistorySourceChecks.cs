using System.Runtime.InteropServices;
using System.Xml;
using Aftercast.ActCompat;
using Advanced_Combat_Tracker;
using Aftercast.Testing;

internal static class HistorySourceChecks
{
    private static void Check(bool value, string message) { if (!value) throw new Exception("History sources: " + message); }
    private static void Reject<T>(Action action) where T : Exception
    { try { action(); } catch (T) { return; } throw new Exception("Expected " + typeof(T).Name); }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-source-proposals-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            CombatFixture.ConfigureCategories();
            var root = ActGlobals.oFormActMain!;
            root.SetEncounter(DateTime.UnixEpoch, "Source", "Target");
            root.AddCombatAction(new(1, false, "", new Dnum(1), DateTime.UnixEpoch, 1, "Strike", "Source", "Physical", "Target"));
            host.EndCombat(); host.ArchiveOperation(3, "", 0, false);
            var index = Path.Combine(directory.FullName, "history/index.xml");
            var xml = new XmlDocument(); xml.Load(index);
            var record = (XmlElement)xml.SelectSingleNode("/Records/Record")!;
            record.SetAttribute("FolderHint", "nested"); record.SetAttribute("CharName", "Hero"); xml.Save(index);
            var page = host.ReadHistory(new());
            var selection = new HistorySelection(page.IndexRevision, [page.Items.Single().Id]);
            var allowed = Directory.CreateDirectory(Path.Combine(directory.FullName, "allowed"));
            var nested = Directory.CreateDirectory(Path.Combine(allowed.FullName, "nested"));
            File.WriteAllText(Path.Combine(nested.FullName, "hidden.log"), "must not be found");
            var main = Path.Combine(allowed.FullName, "Hero-main.log"); File.WriteAllText(main, "0|1\n");
            var query = new HistorySourceQuery([main], [allowed.FullName]);
            var first = host.ReadHistorySources(selection, query);
            Check(first.Candidates.Count == 1 && !first.Ambiguous && !first.Missing && !first.Incomplete && first.RequiresExplicitSelection, "overlap/no automatic selection");
            Check(first.Candidates[0].Origin == (HistorySourceOrigin.Explicit | HistorySourceOrigin.AllowedRoot) &&
                first.Candidates[0].CharacterHintMatches && !first.Candidates[0].FolderHintMatches &&
                first.Candidates[0].WrittenAfterEnd == (File.GetLastWriteTime(main).Ticks > first.End.Ticks), "provenance/advisory hints");
            Check(first.Start.Ticks == page.Items[0].StartTicks && first.End.Ticks == page.Items[0].EndTicks, "history time scope changed");
            Check(host.ListEncounters(0, 16).Total == 1, "proposals changed encounter graph");
            Reject<NotSupportedException>(() => ((IList<HistorySourceCandidate>)first.Candidates)[0] = first.Candidates[0]);
            Refreshes(host, selection, query, first, main);
            AmbiguityAndLimits(host, selection, directory.FullName, allowed.FullName, main);
            OwnershipAndFailures(host, selection, query, index, directory.FullName);
            Console.WriteLine("History source bounded scopes, advisory hints, identity/alias deduplication, metadata refresh, failure/cancel and snapshot ownership passed.");
        }
        finally { directory.Delete(true); }
    }
    private static void Refreshes(HostEnvironment host, HistorySelection selection, HistorySourceQuery query, HistorySourceSnapshot first, string main)
    {
        var same = host.ReadHistorySources(selection, query, first);
        Check(same.Candidates.Single().Change == HistorySourceChange.MetadataUnchanged, "unchanged metadata");
        var modified = File.GetLastWriteTimeUtc(main);
        Thread.Sleep(2); File.WriteAllText(main, "0|2\n"); File.SetLastWriteTimeUtc(main, modified);
        var changed = host.ReadHistorySources(selection, query, same);
        Check(changed.Candidates.Single().Change == HistorySourceChange.MetadataChanged, "same-size same-mtime rewrite change stamp");
        var replacement = main + ".replacement"; File.WriteAllText(replacement, "0|3\n"); File.SetLastWriteTimeUtc(replacement, modified); File.Move(replacement, main, true);
        var replaced = host.ReadHistorySources(selection, query, changed);
        Check(replaced.Candidates.Single().Change == HistorySourceChange.Replaced, "same-size/mtime replacement identity");
        File.Delete(main);
        Check(host.ReadHistorySources(selection, query, replaced).Candidates.Single().State == HistorySourceState.Missing, "removed explicit source hidden");
        File.WriteAllText(main, "0|4\n");
        var rootsOnly = new HistorySourceQuery([], query.Roots);
        var beforeMove = host.ReadHistorySources(selection, rootsOnly);
        var moved = Path.Combine(Path.GetDirectoryName(main)!, "moved.log"); File.Move(main, moved);
        var afterMove = host.ReadHistorySources(selection, rootsOnly, beforeMove);
        Check(afterMove.Candidates.Any(candidate => candidate.Path == moved && candidate.Change == HistorySourceChange.Moved && candidate.MovedFrom == main) &&
            afterMove.Candidates.Any(candidate => candidate.Path == main && candidate.State == HistorySourceState.Missing), "move/missing observations");
        File.Move(moved, main);
        var alias = Path.Combine(Path.GetDirectoryName(main)!, "alias.log");
        var linked = OperatingSystem.IsLinux() ? Link(main, alias) == 0 : OperatingSystem.IsWindows() && CreateHardLink(alias, main, 0);
        Check(linked, "create synthetic hard link");
        var aliases = host.ReadHistorySources(selection, rootsOnly);
        Check(aliases.Candidates.Count == 1 && aliases.Candidates[0].Aliases.Count == 1 && !aliases.Ambiguous, "physical aliases counted as independent matches");
        File.Delete(alias);
    }
    private static void AmbiguityAndLimits(HostEnvironment host, HistorySelection selection, string directory, string allowed, string main)
    {
        var other = Directory.CreateDirectory(Path.Combine(directory, "other"));
        File.WriteAllText(Path.Combine(other.FullName, Path.GetFileName(main)), "other file");
        var ambiguous = host.ReadHistorySources(selection, new([], [allowed, other.FullName]));
        Check(ambiguous.Ambiguous && ambiguous.Candidates.Count == 2 && ambiguous.RequiresExplicitSelection, "same-name matches auto-selected");
        var empty = Directory.CreateDirectory(Path.Combine(directory, "empty"));
        Check(host.ReadHistorySources(selection, new([], [empty.FullName])).Missing, "empty root");
        var bounded = Directory.CreateDirectory(Path.Combine(directory, "bounded"));
        for (var index = 0; index < 4097; index++) File.WriteAllText(Path.Combine(bounded.FullName, index + ".not-matching"), "");
        Reject<InvalidDataException>(() => host.ReadHistorySources(selection, new([], [bounded.FullName])));
        for (var index = 0; index < 65; index++) File.WriteAllText(Path.Combine(empty.FullName, index + ".log"), "");
        Reject<InvalidDataException>(() => host.ReadHistorySources(selection, new([], [empty.FullName])));
        Reject<ArgumentException>(() => host.ReadHistorySources(selection, new([main], [allowed], "../*.log")));
        Reject<ArgumentException>(() => host.ReadHistorySources(selection, new(["relative.log"], [])));
        Reject<ArgumentException>(() => host.ReadHistorySources(selection, new(new string[17], [])));
        Reject<ArgumentException>(() => host.ReadHistorySources(selection, new([], new string[9])));
    }
    private static void OwnershipAndFailures(HostEnvironment host, HistorySelection selection, HistorySourceQuery query, string index, string directory)
    {
        var snapshot = host.ReadHistorySources(selection, query);
        Reject<ArgumentException>(() => host.ReadHistorySources(selection, new(query.Paths, []), snapshot));
        var data = File.ReadAllBytes(index); File.AppendAllText(index, " ");
        try { Reject<InvalidOperationException>(() => host.ReadHistorySources(selection, query, snapshot)); }
        finally { File.WriteAllBytes(index, data); }
        using var cancel = new CancellationTokenSource(); cancel.Cancel();
        Reject<OperationCanceledException>(() => host.ReadHistorySourcesAsync(selection, query, cancellation: cancel.Token).GetAwaiter().GetResult());
        Reject<InvalidOperationException>(() => Task.Run(() => host.ReadHistorySources(selection, query)).GetAwaiter().GetResult());
        var missingRoot = Path.Combine(directory, "missing-root");
        var unavailable = host.ReadHistorySources(selection, new([], [missingRoot, query.Paths[0]]));
        Check(unavailable.Incomplete && unavailable.Diagnostics.Count == 2 && !unavailable.Missing, "root failure hidden as empty search");
        // Windows link creation can require extra privileges; do not alter machine settings for QA.
        if (OperatingSystem.IsLinux()) LinuxSourceBoundaries(host, selection, query, directory);
        var ownedPaths = query.Paths.ToArray(); var ownedRoots = query.Roots.ToArray();
        var pending = host.ReadHistorySourcesAsync(selection, new(ownedPaths, ownedRoots));
        ownedPaths[0] = Path.Combine(directory, "different.log"); ownedRoots[0] = missingRoot;
        var completed = pending.GetAwaiter().GetResult();
        Check(completed.Candidates.Any(candidate => candidate.Path == query.Paths[0]), "caller array mutation changed admitted scope");
        host.Dispose();
        Reject<ObjectDisposedException>(() => host.ReadHistorySources(selection, query, snapshot));
        using var foreign = new HostEnvironment(Directory.CreateDirectory(Path.Combine(directory, "foreign-owner")).FullName);
        Reject<ArgumentException>(() => foreign.ReadHistorySources(selection, query, snapshot));
    }
    private static void LinuxSourceBoundaries(HostEnvironment host, HistorySelection selection, HistorySourceQuery query, string directory)
    {
        var linked = Path.Combine(directory, "linked.log"); File.CreateSymbolicLink(linked, query.Paths[0]);
        Check(host.ReadHistorySources(selection, new([linked], [])).Candidates.Single().State == HistorySourceState.Unavailable, "linked explicit source followed");
        {
            var fifo = Path.Combine(directory, "pipe.log"); Check(MkFifo(fifo, 0x180) == 0, "synthetic FIFO");
            var result = host.ReadHistorySourcesAsync(selection, new([fifo], [])).WaitAsync(TimeSpan.FromSeconds(3)).GetAwaiter().GetResult();
            Check(result.Candidates.Single().State == HistorySourceState.Unavailable, "FIFO data stream opened");
        }
        var rootOnly = new HistorySourceQuery([], query.Roots);
        var rootSnapshot = host.ReadHistorySources(selection, rootOnly);
        var rootPath = query.Roots[0]; var savedRoot = rootPath + ".saved";
        var outside = Directory.CreateDirectory(Path.Combine(directory, "outside"));
        File.WriteAllText(Path.Combine(outside.FullName, Path.GetFileName(query.Paths[0])), "unapproved replacement");
        Directory.Move(rootPath, savedRoot); Directory.CreateSymbolicLink(rootPath, outside.FullName);
        try
        {
            var blocked = host.ReadHistorySources(selection, rootOnly, rootSnapshot);
            Check(blocked.Incomplete && blocked.Candidates.All(candidate => candidate.State == HistorySourceState.Unavailable && candidate.Length is null),
                "previous candidate paths traversed a replaced linked root");
        }
        finally { Directory.Delete(rootPath); Directory.Move(savedRoot, rootPath); }
    }
    [DllImport("libc", EntryPoint = "link", SetLastError = true)]
    private static extern int Link([MarshalAs(UnmanagedType.LPUTF8Str)] string existing, [MarshalAs(UnmanagedType.LPUTF8Str)] string alias);
    [DllImport("libc", EntryPoint = "mkfifo", SetLastError = true)]
    private static extern int MkFifo([MarshalAs(UnmanagedType.LPUTF8Str)] string path, uint mode);
    [DllImport("kernel32.dll", EntryPoint = "CreateHardLinkW", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool CreateHardLink(string alias, string existing, nint reserved);
}
