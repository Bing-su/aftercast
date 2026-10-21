using System.Net;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private static readonly HttpClient pluginVersionClient = new() { Timeout = TimeSpan.FromSeconds(30) };
    public string PluginGetRemoteVersion(int pluginId)
    {
        using var request = new HttpRequestMessage(HttpMethod.Get,
            "https://advancedcombattracker.com/versioncheck/pluginversion/" + pluginId);
        request.Headers.UserAgent.ParseAdd("ACT-Parser");
        try
        {
            using var response = pluginVersionClient.Send(request);
            response.EnsureSuccessStatusCode();
            return response.Content.ReadAsStringAsync().GetAwaiter().GetResult();
        }
        catch (HttpRequestException error) { throw new WebException(error.Message, error); }
        catch (OperationCanceledException error) { throw new WebException(error.Message, error, WebExceptionStatus.Timeout, null); }
    }
}
