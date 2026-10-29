#!/usr/bin/env python3
"""A minimal org.kde.StatusNotifierWatcher: logs registered items and dumps
their properties and menu, so a tray icon can be checked without a desktop."""
import sys
from gi.repository import Gio, GLib

XML = """<node><interface name="org.kde.StatusNotifierWatcher">
<method name="RegisterStatusNotifierItem"><arg type="s" direction="in"/></method>
<method name="RegisterStatusNotifierHost"><arg type="s" direction="in"/></method>
<property name="RegisteredStatusNotifierItems" type="as" access="read"/>
<property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
<property name="ProtocolVersion" type="i" access="read"/>
<signal name="StatusNotifierItemRegistered"><arg type="s"/></signal>
<signal name="StatusNotifierItemUnregistered"><arg type="s"/></signal>
<signal name="StatusNotifierHostRegistered"/>
</interface></node>"""
items = []
log = open(sys.argv[1], "a", buffering=1)

def dump(conn, sender, path):
    def got(c, res):
        try:
            v = c.call_finish(res).unpack()[0]
            keys = ["Id", "Title", "Status", "IconName", "ToolTip", "Menu"]
            log.write("ITEM %s%s: %s pixmaps=%d\n" % (sender, path, {k: v.get(k) for k in keys},
                      len(v.get("IconPixmap", []))))
            menu = v.get("Menu")
            if menu:
                conn.call(sender, menu, "com.canonical.dbusmenu", "GetLayout",
                          GLib.Variant("(iias)", (0, -1, [])), None, 0, -1, None, got_menu)
        except Exception as e:
            log.write("ITEM error %s\n" % e)
    def got_menu(c, res):
        try:
            layout = c.call_finish(res).unpack()[1]
            labels = [ch[1].get("label", "---") for ch in layout[2]]
            log.write("MENU %s\n" % labels)
        except Exception as e:
            log.write("MENU error %s\n" % e)
    conn.call(sender, path, "org.freedesktop.DBus.Properties", "GetAll",
              GLib.Variant("(s)", ("org.kde.StatusNotifierItem",)), None, 0, -1, None, got)

def method(conn, sender, path, iface, name, params, inv):
    if name == "RegisterStatusNotifierItem":
        arg = params.unpack()[0]
        svc, p = (sender, arg) if arg.startswith("/") else (arg, "/StatusNotifierItem")
        items.append(svc + p)
        log.write("REGISTER %s %s\n" % (svc, p))
        GLib.timeout_add(500, lambda: (dump(conn, svc, p), False)[1])
    inv.return_value(None)

def prop(conn, sender, path, iface, name):
    return {"RegisteredStatusNotifierItems": GLib.Variant("as", items),
            "IsStatusNotifierHostRegistered": GLib.Variant("b", True),
            "ProtocolVersion": GLib.Variant("i", 0)}[name]

def acquired(conn, name):
    info = Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0]
    conn.register_object("/StatusNotifierWatcher", info, method, prop, None)

def lost(conn, name):
    pass

def owner_change(conn, sender, path, iface, sig, params):
    name, old, new = params.unpack()
    for it in list(items):
        if it.startswith(name) and new == "":
            items.remove(it)
            log.write("GONE %s\n" % it)

Gio.bus_own_name(Gio.BusType.SESSION, "org.kde.StatusNotifierWatcher", 0, acquired, None, lost)
Gio.bus_get_sync(Gio.BusType.SESSION).signal_subscribe("org.freedesktop.DBus", "org.freedesktop.DBus",
    "NameOwnerChanged", "/org/freedesktop/DBus", None, 0, owner_change)
GLib.MainLoop().run()
