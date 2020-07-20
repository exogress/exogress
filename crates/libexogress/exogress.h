#ifdef __cplusplus
extern "C" {
#endif



#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct {
  int num_threads;
  char *access_token;
  char *endpoint_id;
  char *config_file;
} Config;

bool spawn(const Config *_config);

#ifdef __cplusplus
}
#endif

