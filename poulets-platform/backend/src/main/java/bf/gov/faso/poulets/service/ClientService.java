package bf.gov.faso.poulets.service;

import bf.gov.faso.poulets.model.Client;
import bf.gov.faso.poulets.repository.ClientRepository;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.Optional;
import java.util.UUID;

@Service
public class ClientService {

    private static final Logger log = LoggerFactory.getLogger(ClientService.class);

    private final ClientRepository clientRepository;

    public ClientService(ClientRepository clientRepository) {
        this.clientRepository = clientRepository;
    }

    public Optional<Client> findById(UUID id) {
        return clientRepository.findById(id);
    }

    public Optional<Client> findByUserId(String userId) {
        return clientRepository.findByUserId(userId);
    }

    @Transactional
    public Client register(String userId, String name, String phone, String address) {
        Client client = new Client();
        client.setUserId(userId);
        client.setName(name);
        client.setPhone(phone);
        client.setAddress(address);

        Client saved = clientRepository.save(client);
        log.info("Registered client: id={} name={}", saved.getId(), name);
        return saved;
    }
}
