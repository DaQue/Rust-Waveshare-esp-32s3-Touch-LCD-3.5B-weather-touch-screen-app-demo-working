idf_component_register(
    SRCS "bsp_i2c.c" "bsp_axp2101.cpp" "board_power.c"
    INCLUDE_DIRS "."
    REQUIRES driver freertos XPowersLib
)
