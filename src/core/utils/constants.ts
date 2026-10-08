// Cor da bandeirinha = proporção: azul 16:9, vermelho 4:3, roxo 16:10, rosa ultrawide (21:9),
// marrom super ultrawide (32:9), verde DCI (17:9), amarelo 5:4, laranja 5:3, verde-claro 3:2,
// cinza 11:9 e outras, ciano 1:1.
export const RESOLUTION_LIST = [
    { tag: "DUHD", width: 7680, height: 2160, color: "#b5651d" },
    { tag: "8K FF", width: 8192, height: 4320, color: "#237323" },
    { tag: "8K UHD", width: 7680, height: 4320, color: "#4974ab" },

    { tag: "5K UHD", width: 5120, height: 2880, color: "#4974ab" },

    { tag: "QHD+", width: 3200, height: 1800, color: "#4974ab" },
    { tag: "UW4K", width: 3840, height: 1600, color: "#d77895" },
    { tag: "DQHD", width: 5120, height: 1440, color: "#b5651d" },

    { tag: "WQUXGA", width: 3840, height: 2400, color: "#845a87" },
    { tag: "QUXGA", width: 3200, height: 2400, color: "#c40000" },

    { tag: "5K UW", width: 5120, height: 2160, color: "#d77895" },
    { tag: "4K DCI", width: 4096, height: 2160, color: "#237323" },
    { tag: "4K UHD", width: 3840, height: 2160, color: "#4974ab" },

    { tag: "WQSXGA", width: 3200, height: 2048, color: "#797979" },
    { tag: "QSXGA", width: 2560, height: 2048, color: "#e0c427" },

    { tag: "SQFHD", width: 1920, height: 1920, color: "#63acb7" },

    { tag: "WQXGA", width: 2560, height: 1600, color: "#845a87" },
    { tag: "QXGA", width: 2048, height: 1536, color: "#c40000" },

    { tag: "UWQHD", width: 3440, height: 1440, color: "#d77895" },
    { tag: "WQHD", width: 2560, height: 1440, color: "#4974ab" },

    { tag: "QWXGA", width: 2048, height: 1152, color: "#4974ab" },
    { tag: "DFHD", width: 3840, height: 1080, color: "#b5651d" },
    { tag: "HDV 1080", width: 1440, height: 1080, color: "#c40000" },

    { tag: "WUXGA", width: 1920, height: 1200, color: "#845a87" },
    { tag: "UXGA", width: 1600, height: 1200, color: "#c40000" },

    { tag: "UWFHD", width: 2560, height: 1080, color: "#d77895" },
    { tag: "2K DCI", width: 2048, height: 1080, color: "#237323" },
    { tag: "Full HD", width: 1920, height: 1080, color: "#4974ab" },

    { tag: "WSXGA+", width: 1680, height: 1050, color: "#845a87" },
    { tag: "SXGA+", width: 1400, height: 1050, color: "#c40000" },

    { tag: "SXGA", width: 1280, height: 1024, color: "#e0c427" },

    { tag: "QuadVGA", width: 1280, height: 960, color: "#c40000" },

    { tag: "WSXGA", width: 1600, height: 900, color: "#4974ab" },
    { tag: "WXGA+", width: 1440, height: 900, color: "#845a87" },

    { tag: "XGA+", width: 1152, height: 864, color: "#c40000" },

    { tag: "WXGA 800", width: 1280, height: 800, color: "#845a87" },

    { tag: "FWXGA", width: 1366, height: 768, color: "#4974ab" },
    { tag: "WXGA", width: 1280, height: 768, color: "#e5851f" },
    { tag: "XGA", width: 1024, height: 768, color: "#c40000" },

    { tag: "HD", width: 1280, height: 720, color: "#4974ab" },

    { tag: "WSVGA", width: 1024, height: 600, color: "#797979" },
    { tag: "qHD", width: 960, height: 540, color: "#4974ab" },
    { tag: "SVGA", width: 800, height: 600, color: "#c40000" },

    { tag: "WSVGA PAL", width: 1024, height: 576, color: "#4974ab" },
    { tag: "PAL", width: 768, height: 576, color: "#c40000" },

    { tag: "FWVGA", width: 854, height: 480, color: "#4974ab" },
    { tag: "WVGA", width: 800, height: 480, color: "#e5851f" },
    { tag: "VGA", width: 640, height: 480, color: "#c40000" },

    { tag: "nHD", width: 640, height: 360, color: "#4974ab" },

    { tag: "HVGA", width: 480, height: 320, color: "#6cbe1d" },

    { tag: "SIF", width: 384, height: 288, color: "#c40000" },
    { tag: "CIF", width: 352, height: 288, color: "#797979" },

    { tag: "QVGA", width: 320, height: 240, color: "#c40000" },

    { tag: "CGA", width: 320, height: 200, color: "#845a87" },
    { tag: "QCIF", width: 176, height: 144, color: "#797979" },
    { tag: "QQVGA", width: 160, height: 120, color: "#c40000" },
];

export const FRAMERATE_LIST = [320, 240, 180, 165, 144, 120, 60, 30, 25, 24];

export const DEVICE_LIST_TEST = ["Logitech C920", "EazyCAP Video Capture", "Logitech Brio", "Nikon D5600"];
