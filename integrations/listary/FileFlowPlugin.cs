using System;
using System.Collections.Generic;
using System.IO;
using System.Net.Sockets;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Web.Script.Serialization;
using Listary.FileAppPlugin;

namespace Listary.FileAppPlugin.FileFlow
{
    public sealed class FileFlowPlugin : IFileAppPlugin
    {
        public bool IsOpenedFolderProvider { get { return true; } }
        public bool IsQuickSwitchTarget { get { return false; } }
        public bool IsSharedAcrossApplications { get { return false; } }
        public SearchBarType SearchBarType { get { return SearchBarType.Floating; } }
        public Task<bool> Initialize(IFileAppPluginHost host) { return Task.FromResult(true); }

        public IFileWindow BindFileWindow(IntPtr window)
        {
            if (window == IntPtr.Zero || Native.GetAncestor(window, 2) != window) return null;
            uint pid;
            Native.GetWindowThreadProcessId(window, out pid);
            if (pid == 0) return null;
            IntPtr process = Native.OpenProcess(0x1000, false, pid);
            if (process == IntPtr.Zero) return null;
            try
            {
                var path = new StringBuilder(32768);
                uint length = (uint)path.Capacity;
                if (!Native.QueryFullProcessImageName(process, 0, path, ref length)) return null;
                if (!String.Equals(Path.GetFileName(path.ToString()), "fileflow.exe", StringComparison.OrdinalIgnoreCase)) return null;
                var title = new StringBuilder(256);
                Native.GetWindowText(window, title, title.Capacity);
                if (!String.Equals(title.ToString(), "FileFlow", StringComparison.Ordinal)) return null;
                return new FileFlowWindow(window);
            }
            finally { Native.CloseHandle(process); }
        }
    }

    public sealed class FileFlowWindow : IFileWindow
    {
        public IntPtr Handle { get; private set; }
        public FileFlowWindow(IntPtr window) { Handle = window; }
        public Task<IFileTab> GetCurrentTab() { return Task.FromResult<IFileTab>(new FileFlowTab(Handle)); }
    }

    public sealed class FileFlowTab : IFileTab, IGetFolder
    {
        private readonly IntPtr _window;
        public FileFlowTab(IntPtr window) { _window = window; }
        public Task<string> GetCurrentFolder() { return Task.Run(() => QueryFolder(_window)); }

        private static string QueryFolder(IntPtr window)
        {
            try
            {
                using (var client = new TcpClient())
                {
                    client.ReceiveTimeout = 750;
                    client.SendTimeout = 750;
                    var connect = client.BeginConnect("127.0.0.1", 47719, null, null);
                    using (var signal = connect.AsyncWaitHandle)
                    {
                        if (!signal.WaitOne(300)) return null;
                        client.EndConnect(connect);
                    }
                    using (var stream = client.GetStream())
                    {
                        byte[] request = Encoding.UTF8.GetBytes("fileflow-query:current-directory:v1");
                        stream.Write(request, 0, request.Length);
                        client.Client.Shutdown(SocketShutdown.Send);
                        using (var memory = new MemoryStream())
                        {
                            byte[] buffer = new byte[4096];
                            int count;
                            while ((count = stream.Read(buffer, 0, buffer.Length)) > 0)
                            {
                                if (memory.Length + count > 131072) return null;
                                memory.Write(buffer, 0, count);
                            }
                            var json = Encoding.UTF8.GetString(memory.ToArray());
                            var value = new JavaScriptSerializer().Deserialize<Dictionary<string, object>>(json);
                            if (value == null || !value.ContainsKey("version") || !value.ContainsKey("window_handle") || !value.ContainsKey("path")) return null;
                            if (Convert.ToInt32(value["version"]) != 1 || Convert.ToInt64(value["window_handle"]) != window.ToInt64()) return null;
                            string path = value["path"] as string;
                            return String.IsNullOrEmpty(path) ? null : path;
                        }
                    }
                }
            }
            catch (Exception error)
            {
                // A closed/busy FileFlow or unavailable snapshot is not a folder.
                // Never fall back to stale config paths or touch the clipboard.
                if (error is IOException || error is SocketException || error is ObjectDisposedException || error is InvalidOperationException || error is ArgumentException || error is FormatException || error is OverflowException) return null;
                throw;
            }
        }
    }

    internal static class Native
    {
        [DllImport("user32.dll")] internal static extern IntPtr GetAncestor(IntPtr window, uint flags);
        [DllImport("user32.dll")] internal static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] internal static extern int GetWindowText(IntPtr window, StringBuilder title, int capacity);
        [DllImport("kernel32.dll")] internal static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] internal static extern bool QueryFullProcessImageName(IntPtr process, uint flags, StringBuilder path, ref uint length);
        [DllImport("kernel32.dll")] internal static extern bool CloseHandle(IntPtr handle);
    }
}
