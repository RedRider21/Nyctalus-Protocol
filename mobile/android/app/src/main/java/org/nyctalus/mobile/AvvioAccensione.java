// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
package org.nyctalus.mobile;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/**
 * Avvio all'accensione del telefono, SOLO se l'utente l'ha scelto in «Strumenti»
 * (spento di default): riaccende Nyctalus in Termux e il servizio delle notifiche, così i
 * messaggi arrivano senza dover aprire l'app dopo un riavvio.
 */
public class AvvioAccensione extends BroadcastReceiver {
    @Override
    public void onReceive(Context c, Intent i) {
        if (!Intent.ACTION_BOOT_COMPLETED.equals(i.getAction())) return;
        android.content.SharedPreferences p = c.getSharedPreferences(MainActivity.PREFS, Context.MODE_PRIVATE);
        if (!p.getBoolean("avvio_accensione", false)) return;
        String segreto = p.getString("segreto", "");
        if (segreto.length() >= 32) MainActivity.avviaInTermux(c, segreto);
        ServizioNotifiche.avvia(c);
    }
}
