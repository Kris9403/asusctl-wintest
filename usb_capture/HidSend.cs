using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Collections.Generic;
using Microsoft.Win32.SafeHandles;

public static class HidSend
{
    [StructLayout(LayoutKind.Sequential)]
    struct SP_DEVICE_INTERFACE_DATA
    {
        public int cbSize;
        public Guid InterfaceClassGuid;
        public int Flags;
        public IntPtr Reserved;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct HIDD_ATTRIBUTES
    {
        public int Size;
        public ushort VendorID;
        public ushort ProductID;
        public ushort VersionNumber;
    }

    const uint DIGCF_PRESENT = 0x2;
    const uint DIGCF_DEVICEINTERFACE = 0x10;
    const uint GENERIC_READ = 0x80000000;
    const uint GENERIC_WRITE = 0x40000000;
    const uint FILE_SHARE_READ = 0x1;
    const uint FILE_SHARE_WRITE = 0x2;
    const uint OPEN_EXISTING = 3;

    [DllImport("hid.dll")]
    static extern void HidD_GetHidGuid(out Guid HidGuid);

    [DllImport("hid.dll", SetLastError = true)]
    static extern bool HidD_GetAttributes(SafeFileHandle HidDeviceObject, ref HIDD_ATTRIBUTES Attributes);

    [DllImport("hid.dll", SetLastError = true)]
    static extern bool HidD_SetFeature(SafeFileHandle HidDeviceObject, byte[] lpReportBuffer, uint ReportBufferLength);

    [DllImport("hid.dll", SetLastError = true)]
    static extern bool HidD_GetFeature(SafeFileHandle HidDeviceObject, byte[] lpReportBuffer, uint ReportBufferLength);

    [DllImport("hid.dll", SetLastError = true)]
    static extern bool HidD_SetOutputReport(SafeFileHandle HidDeviceObject, byte[] lpReportBuffer, uint ReportBufferLength);

    [DllImport("setupapi.dll", SetLastError = true)]
    static extern IntPtr SetupDiGetClassDevs(ref Guid ClassGuid, IntPtr Enumerator, IntPtr hwndParent, uint Flags);

    [DllImport("setupapi.dll", SetLastError = true)]
    static extern bool SetupDiEnumDeviceInterfaces(IntPtr DeviceInfoSet, IntPtr DeviceInfoData, ref Guid InterfaceClassGuid, uint MemberIndex, ref SP_DEVICE_INTERFACE_DATA DeviceInterfaceData);

    [DllImport("setupapi.dll", SetLastError = true, CharSet = CharSet.Auto)]
    static extern bool SetupDiGetDeviceInterfaceDetail(IntPtr DeviceInfoSet, ref SP_DEVICE_INTERFACE_DATA DeviceInterfaceData, IntPtr DeviceInterfaceDetailData, uint DeviceInterfaceDetailDataSize, ref uint RequiredSize, IntPtr DeviceInfoData);

    [DllImport("setupapi.dll", SetLastError = true)]
    static extern bool SetupDiDestroyDeviceInfoList(IntPtr DeviceInfoSet);

    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Auto)]
    static extern SafeFileHandle CreateFile(string lpFileName, uint dwDesiredAccess, uint dwShareMode, IntPtr lpSecurityAttributes, uint dwCreationDisposition, uint dwFlagsAndAttributes, IntPtr hTemplateFile);

    public static List<string> EnumeratePaths(ushort vid, ushort pid)
    {
        var results = new List<string>();
        Guid hidGuid;
        HidD_GetHidGuid(out hidGuid);

        IntPtr devInfo = SetupDiGetClassDevs(ref hidGuid, IntPtr.Zero, IntPtr.Zero, DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
        if (devInfo == IntPtr.Zero || devInfo.ToInt64() == -1) return results;

        uint index = 0;
        while (true)
        {
            var ifData = new SP_DEVICE_INTERFACE_DATA();
            ifData.cbSize = Marshal.SizeOf(ifData);
            if (!SetupDiEnumDeviceInterfaces(devInfo, IntPtr.Zero, ref hidGuid, index, ref ifData))
                break;

            uint requiredSize = 0;
            SetupDiGetDeviceInterfaceDetail(devInfo, ref ifData, IntPtr.Zero, 0, ref requiredSize, IntPtr.Zero);

            IntPtr detailBuffer = Marshal.AllocHGlobal((int)requiredSize);
            // cbSize field: 8 on 64-bit, 6 on 32-bit (due to char alignment)
            Marshal.WriteInt32(detailBuffer, IntPtr.Size == 8 ? 8 : 6);

            if (SetupDiGetDeviceInterfaceDetail(devInfo, ref ifData, detailBuffer, requiredSize, ref requiredSize, IntPtr.Zero))
            {
                string path = Marshal.PtrToStringAuto(detailBuffer + 4);

                var handle = CreateFile(path, 0, FILE_SHARE_READ | FILE_SHARE_WRITE, IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
                if (!handle.IsInvalid)
                {
                    var attrs = new HIDD_ATTRIBUTES();
                    attrs.Size = Marshal.SizeOf(attrs);
                    if (HidD_GetAttributes(handle, ref attrs))
                    {
                        if (attrs.VendorID == vid && attrs.ProductID == pid)
                            results.Add(path);
                    }
                    handle.Close();
                }
            }
            Marshal.FreeHGlobal(detailBuffer);
            index++;
        }

        SetupDiDestroyDeviceInfoList(devInfo);
        return results;
    }

    public static bool TrySetFeature(string path, byte[] report)
    {
        var handle = CreateFile(path, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
        if (handle.IsInvalid) return false;
        bool ok = HidD_SetFeature(handle, report, (uint)report.Length);
        handle.Close();
        return ok;
    }

    public static bool TrySetOutputReport(string path, byte[] report)
    {
        var handle = CreateFile(path, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
        if (handle.IsInvalid) return false;
        bool ok = HidD_SetOutputReport(handle, report, (uint)report.Length);
        handle.Close();
        return ok;
    }

    public static int LastError()
    {
        return Marshal.GetLastWin32Error();
    }

    // Persistent-handle API for animation loops: opening a fresh handle per
    // frame (as TrySetFeature does) is wasteful at 20-30fps. Open once,
    // reuse for the whole animation, close when done.
    public static SafeFileHandle OpenPersistent(string path)
    {
        return CreateFile(path, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, IntPtr.Zero, OPEN_EXISTING, 0, IntPtr.Zero);
    }

    public static bool SetFeatureOnHandle(SafeFileHandle handle, byte[] report)
    {
        return HidD_SetFeature(handle, report, (uint)report.Length);
    }
}
