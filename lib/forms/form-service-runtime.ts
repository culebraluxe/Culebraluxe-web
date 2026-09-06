import 'server-only'

import { SqlContractRepository } from '@/db/contract-service-repository'
import { SqlFirmRepository } from '@/db/firm-service-repository'
import { SqlPersonRepository } from '@/db/person-service-repository'
import { SqlPropertyRepository } from '@/db/property-service-repository'
import { SqlShowingRepository } from '@/db/showing-service-repository'
import { composeCoreServices } from '@/services/composition'
import { ShowingService } from '@/services/showing'

export const formCoreServices = composeCoreServices({
  person: new SqlPersonRepository(),
  firm: new SqlFirmRepository(),
  property: new SqlPropertyRepository(),
  contract: new SqlContractRepository(),
})

export const formShowingService = formCoreServices.registry.register(
  new ShowingService(new SqlShowingRepository(), { router: formCoreServices.registry }),
)
