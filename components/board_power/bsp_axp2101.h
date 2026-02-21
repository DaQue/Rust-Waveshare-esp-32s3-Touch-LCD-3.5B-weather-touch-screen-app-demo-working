#ifndef BSP_AXP2101_H
#define BSP_AXP2101_H

#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

esp_err_t bsp_axp2101_init(void);
void pmu_isr_handler(void);

#ifdef __cplusplus
}
#endif

#endif
