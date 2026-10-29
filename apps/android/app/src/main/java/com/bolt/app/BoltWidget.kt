package com.bolt.app

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.service.quicksettings.TileService
import android.widget.RemoteViews

/**
 * Home-screen widget (2x1): a rounded dark body with two round keys. The power key turns Bolt off or on (its icon is
 * green while Bolt is on, grey when off); the send key opens the file picker.
 */
class BoltWidget : AppWidgetProvider() {
    override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
        ids.forEach { manager.updateAppWidget(it, views(context, Engine.isRunning())) }
    }

    companion object {
        const val ACTION_PICK_FILES = "com.bolt.PICK_FILES"

        /** Redraw every placed widget (and the quick-settings tile) for the current state. */
        fun refresh(ctx: Context, running: Boolean = Engine.isRunning()) {
            val app = ctx.applicationContext
            val manager = AppWidgetManager.getInstance(app)
            manager.getAppWidgetIds(ComponentName(app, BoltWidget::class.java)).forEach {
                manager.updateAppWidget(it, views(app, running))
            }
            runCatching { TileService.requestListeningState(app, ComponentName(app, BoltTile::class.java)) }
        }

        private fun views(ctx: Context, running: Boolean): RemoteViews {
            val v = RemoteViews(ctx.packageName, R.layout.widget_bolt)
            // no lights: the power icon itself shows the state
            v.setInt(R.id.widget_power_icon, "setColorFilter", if (running) 0xFF32D74B.toInt() else 0xFF8E8E93.toInt())
            v.setInt(R.id.widget_send_icon, "setColorFilter", 0xFFDADADE.toInt())
            v.setContentDescription(R.id.widget_toggle, if (running) "Bolt is on. Tap to turn off" else "Bolt is off. Tap to turn on")
            if (running) {
                v.setOnClickPendingIntent(
                    R.id.widget_toggle,
                    PendingIntent.getService(ctx, 11, Intent(ctx, TransferService::class.java).setAction(TransferService.ACTION_STOP), PendingIntent.FLAG_IMMUTABLE),
                )
            } else {
                v.setOnClickPendingIntent(
                    R.id.widget_toggle,
                    PendingIntent.getForegroundService(ctx, 12, Intent(ctx, TransferService::class.java), PendingIntent.FLAG_IMMUTABLE),
                )
            }
            // Send: open Bolt straight into the file picker, then pick the device on the card
            v.setOnClickPendingIntent(
                R.id.widget_send,
                PendingIntent.getActivity(
                    ctx, 10,
                    Intent(ctx, MainActivity::class.java).setAction(ACTION_PICK_FILES).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP),
                    PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                ),
            )
            return v
        }
    }
}
