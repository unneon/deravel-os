macro defines($(#define $name:ident $value:literal)*) {
    $(#[allow(dead_code)]
    pub const $name: u16 = $value;)*
}

defines! {
    #define EV_SYN			0x00
    #define EV_KEY			0x01
    #define EV_REL			0x02
    #define EV_ABS			0x03
    #define EV_MSC			0x04
    #define EV_SW			0x05
    #define EV_LED			0x11
    #define EV_SND			0x12
    #define EV_REP			0x14
    #define EV_FF			0x15
    #define EV_PWR			0x16
    #define EV_FF_STATUS		0x17
    #define EV_MAX			0x1f
}

defines! {
    #define KEY_RESERVED		0
    #define KEY_ESC			1
    #define KEY_1			2
    #define KEY_2			3
    #define KEY_3			4
    #define KEY_4			5
    #define KEY_5			6
    #define KEY_6			7
    #define KEY_7			8
    #define KEY_8			9
    #define KEY_9			10
    #define KEY_0			11
    #define KEY_MINUS		12
    #define KEY_EQUAL		13
    #define KEY_BACKSPACE		14
    #define KEY_TAB			15
    #define KEY_Q			16
    #define KEY_W			17
    #define KEY_E			18
    #define KEY_R			19
    #define KEY_T			20
    #define KEY_Y			21
    #define KEY_U			22
    #define KEY_I			23
    #define KEY_O			24
    #define KEY_P			25
    #define KEY_LEFTBRACE		26
    #define KEY_RIGHTBRACE		27
    #define KEY_ENTER		28
    #define KEY_LEFTCTRL		29
    #define KEY_A			30
    #define KEY_S			31
    #define KEY_D			32
    #define KEY_F			33
    #define KEY_G			34
    #define KEY_H			35
    #define KEY_J			36
    #define KEY_K			37
    #define KEY_L			38
    #define KEY_SEMICOLON		39
    #define KEY_APOSTROPHE		40
    #define KEY_GRAVE		41
    #define KEY_LEFTSHIFT		42
    #define KEY_BACKSLASH		43
    #define KEY_Z			44
    #define KEY_X			45
    #define KEY_C			46
    #define KEY_V			47
    #define KEY_B			48
    #define KEY_N			49
    #define KEY_M			50
    #define KEY_COMMA		51
    #define KEY_DOT			52
    #define KEY_SLASH		53
    #define KEY_RIGHTSHIFT		54
    #define KEY_KPASTERISK		55
    #define KEY_LEFTALT		56
    #define KEY_SPACE		57
    #define KEY_CAPSLOCK		58
    #define KEY_F1			59
    #define KEY_F2			60
    #define KEY_F3			61
    #define KEY_F4			62
    #define KEY_F5			63
    #define KEY_F6			64
    #define KEY_F7			65
    #define KEY_F8			66
    #define KEY_F9			67
    #define KEY_F10			68
    #define KEY_NUMLOCK		69
    #define KEY_SCROLLLOCK		70
    #define KEY_KP7			71
    #define KEY_KP8			72
    #define KEY_KP9			73
    #define KEY_KPMINUS		74
    #define KEY_KP4			75
    #define KEY_KP5			76
    #define KEY_KP6			77
    #define KEY_KPPLUS		78
    #define KEY_KP1			79
    #define KEY_KP2			80
    #define KEY_KP3			81
    #define KEY_KP0			82
    #define KEY_KPDOT		83
}

defines! {
    #define BTN_MOUSE		0x110
    #define BTN_LEFT		0x110
    #define BTN_RIGHT		0x111
    #define BTN_MIDDLE		0x112
    #define BTN_SIDE		0x113
    #define BTN_EXTRA		0x114
    #define BTN_FORWARD		0x115
    #define BTN_BACK		0x116
    #define BTN_TASK		0x117
}

defines! {
    #define REL_X			0x00
    #define REL_Y			0x01
    #define REL_Z			0x02
    #define REL_RX			0x03
    #define REL_RY			0x04
    #define REL_RZ			0x05
    #define REL_HWHEEL		0x06
    #define REL_DIAL		0x07
    #define REL_WHEEL		0x08
    #define REL_MISC		0x09
    #define REL_RESERVED		0x0a
    #define REL_WHEEL_HI_RES	0x0b
    #define REL_HWHEEL_HI_RES	0x0c
}
