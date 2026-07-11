package io.jimin.os

import android.app.Activity
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning

@TauriPlugin
class JiminQrScannerPlugin(private val activity: Activity) : Plugin(activity) {
    private var scanInProgress = false

    @Command
    fun scan(invoke: Invoke) {
        if (scanInProgress) {
            invoke.reject("A QR scan is already active.")
            return
        }

        scanInProgress = true
        val options = GmsBarcodeScannerOptions.Builder()
            .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
            .enableAutoZoom()
            .build()
        val scanner = GmsBarcodeScanning.getClient(activity, options)

        scanner.startScan()
            .addOnSuccessListener { barcode ->
                scanInProgress = false
                val content = barcode.rawValue
                if (content.isNullOrBlank()) {
                    invoke.reject("The QR code did not contain a readable value.")
                    return@addOnSuccessListener
                }
                invoke.resolve(JSObject().put("content", content))
            }
            .addOnCanceledListener {
                scanInProgress = false
                invoke.resolve(JSObject().put("content", null))
            }
            .addOnFailureListener { error ->
                scanInProgress = false
                invoke.reject(error.message ?: "The QR scanner could not be opened.")
            }
    }
}
