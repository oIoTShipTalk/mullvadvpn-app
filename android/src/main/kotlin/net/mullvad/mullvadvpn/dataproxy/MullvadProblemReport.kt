package net.mullvad.mullvadvpn.dataproxy

const val PROBLEM_REPORT_PATH = "/data/data/net.mullvad.mullvadvpn/problem_report.txt"

class MullvadProblemReport {
    var userEmail = ""
    var userMessage = ""

    init {
        System.loadLibrary("mullvad_jni")
    }

    private external fun collectReport(reportPath: String): Boolean
}
