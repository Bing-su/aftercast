namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private volatile int globalTimeSorter;

    // The reader and plugins share this sequence; retain the original writable property contract.
    public int GlobalTimeSorter { get => globalTimeSorter; set => globalTimeSorter = value; }
}
