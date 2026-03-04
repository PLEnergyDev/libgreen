#ifndef GREEN_H
#define GREEN_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void* MeasurementContext;

MeasurementContext measure_start(const char* events);
void measure_stop(MeasurementContext context);

#ifdef __cplusplus
}
#endif

#endif

