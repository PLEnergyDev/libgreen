#ifndef GREEN_H
#define GREEN_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void* MeasurementHandle;

MeasurementHandle measure_start(const char* events);
void              measure_stop(MeasurementHandle handle);

#ifdef __cplusplus
}
#endif

#endif
