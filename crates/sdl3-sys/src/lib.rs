#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused_parens)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
/**< window is in fullscreen mode */
pub const SDL_WINDOW_FULLSCREEN: u64 = 0x0000000000000001;
/**< window usable with OpenGL context */
pub const SDL_WINDOW_OPENGL: u64 = 0x0000000000000002;
/**< window is occluded */
pub const SDL_WINDOW_OCCLUDED: u64 = 0x0000000000000004;
/**< window is neither mapped onto the desktop nor shown in the taskbar/dock/window list; SDL_ShowWindow() is required for it to become visible */
pub const SDL_WINDOW_HIDDEN: u64 = 0x0000000000000008;
/**< no window decoration */
pub const SDL_WINDOW_BORDERLESS: u64 = 0x0000000000000010;
/**< window can be resized */
pub const SDL_WINDOW_RESIZABLE: u64 = 0x0000000000000020;
/**< window is minimized */
pub const SDL_WINDOW_MINIMIZED: u64 = 0x0000000000000040;
/**< window is maximized */
pub const SDL_WINDOW_MAXIMIZED: u64 = 0x0000000000000080;
/**< window has grabbed mouse input */
pub const SDL_WINDOW_MOUSE_GRABBED: u64 = 0x0000000000000100;
/**< window has input focus */
pub const SDL_WINDOW_INPUT_FOCUS: u64 = 0x0000000000000200;
/**< window has mouse focus */
pub const SDL_WINDOW_MOUSE_FOCUS: u64 = 0x0000000000000400;
/**< window not created by SDL */
pub const SDL_WINDOW_EXTERNAL: u64 = 0x0000000000000800;
/**< window is modal */
pub const SDL_WINDOW_MODAL: u64 = 0x0000000000001000;
/**< window uses high pixel density back buffer if possible */
pub const SDL_WINDOW_HIGH_PIXEL_DENSITY: u64 = 0x0000000000002000;
/**< window has mouse captured (unrelated to MOUSE_GRABBED) */
pub const SDL_WINDOW_MOUSE_CAPTURE: u64 = 0x0000000000004000;
/**< window has relative mode enabled */
pub const SDL_WINDOW_MOUSE_RELATIVE_MODE: u64 = 0x0000000000008000;
/**< window should always be above others */
pub const SDL_WINDOW_ALWAYS_ON_TOP: u64 = 0x0000000000010000;
/**< window should be treated as a utility window, not showing in the task bar and window list */
pub const SDL_WINDOW_UTILITY: u64 = 0x0000000000020000;
/**< window should be treated as a tooltip and does not get mouse or keyboard focus, requires a parent window */
pub const SDL_WINDOW_TOOLTIP: u64 = 0x0000000000040000;
/**< window should be treated as a popup menu, requires a parent window */
pub const SDL_WINDOW_POPUP_MENU: u64 = 0x0000000000080000;
/**< window has grabbed keyboard input */
pub const SDL_WINDOW_KEYBOARD_GRABBED: u64 = 0x0000000000100000;
/**< window usable for Vulkan surface */
pub const SDL_WINDOW_VULKAN: u64 = 0x0000000010000000;
/**< window usable for Metal view */
pub const SDL_WINDOW_METAL: u64 = 0x0000000020000000;
/**< window with transparent buffer */
pub const SDL_WINDOW_TRANSPARENT: u64 = 0x0000000040000000;
/**< window should not be focusable */
pub const SDL_WINDOW_NOT_FOCUSABLE: u64 = 0x0000000080000000;

/// Unused (do not remove)
pub const SDL_EVENT_FIRST: SDL_EventType = (0 as Uint32);
/// User-requested quit
pub const SDL_EVENT_QUIT: SDL_EventType = (0x100 as Uint32);
/// The application is being terminated by the OS. This event must be handled in a callback set with [`SDL_AddEventWatch()`].
/// Called on iOS in applicationWillTerminate()
/// Called on Android in onDestroy()
pub const SDL_EVENT_TERMINATING: SDL_EventType = (257 as Uint32);
/// The application is low on memory, free memory if possible. This event must be handled in a callback set with [`SDL_AddEventWatch()`].
/// Called on iOS in applicationDidReceiveMemoryWarning()
/// Called on Android in onTrimMemory()
pub const SDL_EVENT_LOW_MEMORY: SDL_EventType = (258 as Uint32);
/// The application is about to enter the background. This event must be handled in a callback set with [`SDL_AddEventWatch()`].
/// Called on iOS in applicationWillResignActive()
/// Called on Android in onPause()
pub const SDL_EVENT_WILL_ENTER_BACKGROUND: SDL_EventType = (259 as Uint32);
/// The application did enter the background and may not get CPU for some time. This event must be handled in a callback set with [`SDL_AddEventWatch()`].
/// Called on iOS in applicationDidEnterBackground()
/// Called on Android in onPause()
pub const SDL_EVENT_DID_ENTER_BACKGROUND: SDL_EventType = (260 as Uint32);
/// The application is about to enter the foreground. This event must be handled in a callback set with [`SDL_AddEventWatch()`].
/// Called on iOS in applicationWillEnterForeground()
/// Called on Android in onResume()
pub const SDL_EVENT_WILL_ENTER_FOREGROUND: SDL_EventType = (261 as Uint32);
/// The application is now interactive. This event must be handled in a callback set with [`SDL_AddEventWatch()`].
/// Called on iOS in applicationDidBecomeActive()
/// Called on Android in onResume()
pub const SDL_EVENT_DID_ENTER_FOREGROUND: SDL_EventType = (262 as Uint32);
/// The user's locale preferences have changed.
pub const SDL_EVENT_LOCALE_CHANGED: SDL_EventType = (263 as Uint32);
/// The system theme changed
pub const SDL_EVENT_SYSTEM_THEME_CHANGED: SDL_EventType = (264 as Uint32);
/// Display orientation has changed to data1
pub const SDL_EVENT_DISPLAY_ORIENTATION: SDL_EventType = (0x151 as Uint32);
/// Display has been added to the system
pub const SDL_EVENT_DISPLAY_ADDED: SDL_EventType = (338 as Uint32);
/// Display has been removed from the system
pub const SDL_EVENT_DISPLAY_REMOVED: SDL_EventType = (339 as Uint32);
/// Display has changed position
pub const SDL_EVENT_DISPLAY_MOVED: SDL_EventType = (340 as Uint32);
/// Display has changed desktop mode
pub const SDL_EVENT_DISPLAY_DESKTOP_MODE_CHANGED: SDL_EventType = (341 as Uint32);
/// Display has changed current mode
pub const SDL_EVENT_DISPLAY_CURRENT_MODE_CHANGED: SDL_EventType = (342 as Uint32);
/// Display has changed content scale
pub const SDL_EVENT_DISPLAY_CONTENT_SCALE_CHANGED: SDL_EventType = (343 as Uint32);
pub const SDL_EVENT_DISPLAY_FIRST: SDL_EventType = SDL_EVENT_DISPLAY_ORIENTATION;
pub const SDL_EVENT_DISPLAY_LAST: SDL_EventType = SDL_EVENT_DISPLAY_CONTENT_SCALE_CHANGED;
/// Window has been shown
pub const SDL_EVENT_WINDOW_SHOWN: SDL_EventType = (0x202 as Uint32);
/// Window has been hidden
pub const SDL_EVENT_WINDOW_HIDDEN: SDL_EventType = (515 as Uint32);
/// Window has been exposed and should be redrawn, and can be redrawn directly from event watchers for this event
pub const SDL_EVENT_WINDOW_EXPOSED: SDL_EventType = (516 as Uint32);
/// Window has been moved to data1, data2
pub const SDL_EVENT_WINDOW_MOVED: SDL_EventType = (517 as Uint32);
/// Window has been resized to data1xdata2
pub const SDL_EVENT_WINDOW_RESIZED: SDL_EventType = (518 as Uint32);
/// The pixel size of the window has changed to data1xdata2
pub const SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED: SDL_EventType = (519 as Uint32);
/// The pixel size of a Metal view associated with the window has changed
pub const SDL_EVENT_WINDOW_METAL_VIEW_RESIZED: SDL_EventType = (520 as Uint32);
/// Window has been minimized
pub const SDL_EVENT_WINDOW_MINIMIZED: SDL_EventType = (521 as Uint32);
/// Window has been maximized
pub const SDL_EVENT_WINDOW_MAXIMIZED: SDL_EventType = (522 as Uint32);
/// Window has been restored to normal size and position
pub const SDL_EVENT_WINDOW_RESTORED: SDL_EventType = (523 as Uint32);
/// Window has gained mouse focus
pub const SDL_EVENT_WINDOW_MOUSE_ENTER: SDL_EventType = (524 as Uint32);
/// Window has lost mouse focus
pub const SDL_EVENT_WINDOW_MOUSE_LEAVE: SDL_EventType = (525 as Uint32);
/// Window has gained keyboard focus
pub const SDL_EVENT_WINDOW_FOCUS_GAINED: SDL_EventType = (526 as Uint32);
/// Window has lost keyboard focus
pub const SDL_EVENT_WINDOW_FOCUS_LOST: SDL_EventType = (527 as Uint32);
/// The window manager requests that the window be closed
pub const SDL_EVENT_WINDOW_CLOSE_REQUESTED: SDL_EventType = (528 as Uint32);
/// Window had a hit test that wasn't [`SDL_HITTEST_NORMAL`]
pub const SDL_EVENT_WINDOW_HIT_TEST: SDL_EventType = (529 as Uint32);
/// The ICC profile of the window's display has changed
pub const SDL_EVENT_WINDOW_ICCPROF_CHANGED: SDL_EventType = (530 as Uint32);
/// Window has been moved to display data1
pub const SDL_EVENT_WINDOW_DISPLAY_CHANGED: SDL_EventType = (531 as Uint32);
/// Window display scale has been changed
pub const SDL_EVENT_WINDOW_DISPLAY_SCALE_CHANGED: SDL_EventType = (532 as Uint32);
/// The window safe area has been changed
pub const SDL_EVENT_WINDOW_SAFE_AREA_CHANGED: SDL_EventType = (533 as Uint32);
/// The window has been occluded
pub const SDL_EVENT_WINDOW_OCCLUDED: SDL_EventType = (534 as Uint32);
/// The window has entered fullscreen mode
pub const SDL_EVENT_WINDOW_ENTER_FULLSCREEN: SDL_EventType = (535 as Uint32);
/// The window has left fullscreen mode
pub const SDL_EVENT_WINDOW_LEAVE_FULLSCREEN: SDL_EventType = (536 as Uint32);
/// The window with the associated ID is being or has been destroyed. If this message is being handled
/// in an event watcher, the window handle is still valid and can still be used to retrieve any properties
/// associated with the window. Otherwise, the handle has already been destroyed and all resources
/// associated with it are invalid
pub const SDL_EVENT_WINDOW_DESTROYED: SDL_EventType = (537 as Uint32);
/// Window HDR properties have changed
pub const SDL_EVENT_WINDOW_HDR_STATE_CHANGED: SDL_EventType = (538 as Uint32);
pub const SDL_EVENT_WINDOW_FIRST: SDL_EventType = SDL_EVENT_WINDOW_SHOWN;
pub const SDL_EVENT_WINDOW_LAST: SDL_EventType = SDL_EVENT_WINDOW_HDR_STATE_CHANGED;
/// Key pressed
pub const SDL_EVENT_KEY_DOWN: SDL_EventType = (0x300 as Uint32);
/// Key released
pub const SDL_EVENT_KEY_UP: SDL_EventType = (769 as Uint32);
/// Keyboard text editing (composition)
pub const SDL_EVENT_TEXT_EDITING: SDL_EventType = (770 as Uint32);
/// Keyboard text input
pub const SDL_EVENT_TEXT_INPUT: SDL_EventType = (771 as Uint32);
/// Keymap changed due to a system event such as an
/// input language or keyboard layout change.
pub const SDL_EVENT_KEYMAP_CHANGED: SDL_EventType = (772 as Uint32);
/// A new keyboard has been inserted into the system
pub const SDL_EVENT_KEYBOARD_ADDED: SDL_EventType = (773 as Uint32);
/// A keyboard has been removed
pub const SDL_EVENT_KEYBOARD_REMOVED: SDL_EventType = (774 as Uint32);
/// Keyboard text editing candidates
pub const SDL_EVENT_TEXT_EDITING_CANDIDATES: SDL_EventType = (775 as Uint32);
/// Mouse moved
pub const SDL_EVENT_MOUSE_MOTION: SDL_EventType = (0x400 as Uint32);
/// Mouse button pressed
pub const SDL_EVENT_MOUSE_BUTTON_DOWN: SDL_EventType = (1025 as Uint32);
/// Mouse button released
pub const SDL_EVENT_MOUSE_BUTTON_UP: SDL_EventType = (1026 as Uint32);
/// Mouse wheel motion
pub const SDL_EVENT_MOUSE_WHEEL: SDL_EventType = (1027 as Uint32);
/// A new mouse has been inserted into the system
pub const SDL_EVENT_MOUSE_ADDED: SDL_EventType = (1028 as Uint32);
/// A mouse has been removed
pub const SDL_EVENT_MOUSE_REMOVED: SDL_EventType = (1029 as Uint32);
/// Joystick axis motion
pub const SDL_EVENT_JOYSTICK_AXIS_MOTION: SDL_EventType = (0x600 as Uint32);
/// Joystick trackball motion
pub const SDL_EVENT_JOYSTICK_BALL_MOTION: SDL_EventType = (1537 as Uint32);
/// Joystick hat position change
pub const SDL_EVENT_JOYSTICK_HAT_MOTION: SDL_EventType = (1538 as Uint32);
/// Joystick button pressed
pub const SDL_EVENT_JOYSTICK_BUTTON_DOWN: SDL_EventType = (1539 as Uint32);
/// Joystick button released
pub const SDL_EVENT_JOYSTICK_BUTTON_UP: SDL_EventType = (1540 as Uint32);
/// A new joystick has been inserted into the system
pub const SDL_EVENT_JOYSTICK_ADDED: SDL_EventType = (1541 as Uint32);
/// An opened joystick has been removed
pub const SDL_EVENT_JOYSTICK_REMOVED: SDL_EventType = (1542 as Uint32);
/// Joystick battery level change
pub const SDL_EVENT_JOYSTICK_BATTERY_UPDATED: SDL_EventType = (1543 as Uint32);
/// Joystick update is complete
pub const SDL_EVENT_JOYSTICK_UPDATE_COMPLETE: SDL_EventType = (1544 as Uint32);
/// Gamepad axis motion
pub const SDL_EVENT_GAMEPAD_AXIS_MOTION: SDL_EventType = (0x650 as Uint32);
/// Gamepad button pressed
pub const SDL_EVENT_GAMEPAD_BUTTON_DOWN: SDL_EventType = (1617 as Uint32);
/// Gamepad button released
pub const SDL_EVENT_GAMEPAD_BUTTON_UP: SDL_EventType = (1618 as Uint32);
/// A new gamepad has been inserted into the system
pub const SDL_EVENT_GAMEPAD_ADDED: SDL_EventType = (1619 as Uint32);
/// A gamepad has been removed
pub const SDL_EVENT_GAMEPAD_REMOVED: SDL_EventType = (1620 as Uint32);
/// The gamepad mapping was updated
pub const SDL_EVENT_GAMEPAD_REMAPPED: SDL_EventType = (1621 as Uint32);
/// Gamepad touchpad was touched
pub const SDL_EVENT_GAMEPAD_TOUCHPAD_DOWN: SDL_EventType = (1622 as Uint32);
/// Gamepad touchpad finger was moved
pub const SDL_EVENT_GAMEPAD_TOUCHPAD_MOTION: SDL_EventType = (1623 as Uint32);
/// Gamepad touchpad finger was lifted
pub const SDL_EVENT_GAMEPAD_TOUCHPAD_UP: SDL_EventType = (1624 as Uint32);
/// Gamepad sensor was updated
pub const SDL_EVENT_GAMEPAD_SENSOR_UPDATE: SDL_EventType = (1625 as Uint32);
/// Gamepad update is complete
pub const SDL_EVENT_GAMEPAD_UPDATE_COMPLETE: SDL_EventType = (1626 as Uint32);
/// Gamepad Steam handle has changed
pub const SDL_EVENT_GAMEPAD_STEAM_HANDLE_UPDATED: SDL_EventType = (1627 as Uint32);
pub const SDL_EVENT_FINGER_DOWN: SDL_EventType = (0x700 as Uint32);
pub const SDL_EVENT_FINGER_UP: SDL_EventType = (1793 as Uint32);
pub const SDL_EVENT_FINGER_MOTION: SDL_EventType = (1794 as Uint32);
pub const SDL_EVENT_FINGER_CANCELED: SDL_EventType = (1795 as Uint32);
/// The clipboard or primary selection changed
pub const SDL_EVENT_CLIPBOARD_UPDATE: SDL_EventType = (0x900 as Uint32);
/// The system requests a file open
pub const SDL_EVENT_DROP_FILE: SDL_EventType = (0x1000 as Uint32);
/// text/plain drag-and-drop event
pub const SDL_EVENT_DROP_TEXT: SDL_EventType = (4097 as Uint32);
/// A new set of drops is beginning (NULL filename)
pub const SDL_EVENT_DROP_BEGIN: SDL_EventType = (4098 as Uint32);
/// Current set of drops is now complete (NULL filename)
pub const SDL_EVENT_DROP_COMPLETE: SDL_EventType = (4099 as Uint32);
/// Position while moving over the window
pub const SDL_EVENT_DROP_POSITION: SDL_EventType = (4100 as Uint32);
/// A new audio device is available
pub const SDL_EVENT_AUDIO_DEVICE_ADDED: SDL_EventType = (0x1100 as Uint32);
/// An audio device has been removed.
pub const SDL_EVENT_AUDIO_DEVICE_REMOVED: SDL_EventType = (4353 as Uint32);
/// An audio device's format has been changed by the system.
pub const SDL_EVENT_AUDIO_DEVICE_FORMAT_CHANGED: SDL_EventType = (4354 as Uint32);
/// A sensor was updated
pub const SDL_EVENT_SENSOR_UPDATE: SDL_EventType = (0x1200 as Uint32);
/// Pressure-sensitive pen has become available
pub const SDL_EVENT_PEN_PROXIMITY_IN: SDL_EventType = (0x1300 as Uint32);
/// Pressure-sensitive pen has become unavailable
pub const SDL_EVENT_PEN_PROXIMITY_OUT: SDL_EventType = (4865 as Uint32);
/// Pressure-sensitive pen touched drawing surface
pub const SDL_EVENT_PEN_DOWN: SDL_EventType = (4866 as Uint32);
/// Pressure-sensitive pen stopped touching drawing surface
pub const SDL_EVENT_PEN_UP: SDL_EventType = (4867 as Uint32);
/// Pressure-sensitive pen button pressed
pub const SDL_EVENT_PEN_BUTTON_DOWN: SDL_EventType = (4868 as Uint32);
/// Pressure-sensitive pen button released
pub const SDL_EVENT_PEN_BUTTON_UP: SDL_EventType = (4869 as Uint32);
/// Pressure-sensitive pen is moving on the tablet
pub const SDL_EVENT_PEN_MOTION: SDL_EventType = (4870 as Uint32);
/// Pressure-sensitive pen angle/pressure/etc changed
pub const SDL_EVENT_PEN_AXIS: SDL_EventType = (4871 as Uint32);
/// A new camera device is available
pub const SDL_EVENT_CAMERA_DEVICE_ADDED: SDL_EventType = (0x1400 as Uint32);
/// A camera device has been removed.
pub const SDL_EVENT_CAMERA_DEVICE_REMOVED: SDL_EventType = (5121 as Uint32);
/// A camera device has been approved for use by the user.
pub const SDL_EVENT_CAMERA_DEVICE_APPROVED: SDL_EventType = (5122 as Uint32);
/// A camera device has been denied for use by the user.
pub const SDL_EVENT_CAMERA_DEVICE_DENIED: SDL_EventType = (5123 as Uint32);
/// The render targets have been reset and their contents need to be updated
pub const SDL_EVENT_RENDER_TARGETS_RESET: SDL_EventType = (0x2000 as Uint32);
/// The device has been reset and all textures need to be recreated
pub const SDL_EVENT_RENDER_DEVICE_RESET: SDL_EventType = (8193 as Uint32);
/// The device has been lost and can't be recovered.
pub const SDL_EVENT_RENDER_DEVICE_LOST: SDL_EventType = (8194 as Uint32);
pub const SDL_EVENT_PRIVATE0: SDL_EventType = (0x4000 as Uint32);
pub const SDL_EVENT_PRIVATE1: SDL_EventType = (16385 as Uint32);
pub const SDL_EVENT_PRIVATE2: SDL_EventType = (16386 as Uint32);
pub const SDL_EVENT_PRIVATE3: SDL_EventType = (16387 as Uint32);
/// Signals the end of an event poll cycle
pub const SDL_EVENT_POLL_SENTINEL: SDL_EventType = (0x7f00 as Uint32);
///  Events [`SDL_EVENT_USER`] through [`SDL_EVENT_LAST`] are for your use,
/// and should be allocated with [`SDL_RegisterEvents()`]
pub const SDL_EVENT_USER: SDL_EventType = (0x8000 as Uint32);
/// *  This last event is only for bounding internal arrays
pub const SDL_EVENT_LAST: SDL_EventType = (0xffff as Uint32);
pub const SDL_EVENT_ENUM_PADDING: SDL_EventType = (0x7fffffff as Uint32);
