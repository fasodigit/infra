package bf.gov.faso.poulets;

import bf.gov.faso.poulets.model.*;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;

import java.time.Instant;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Unit tests for poulets-api domain models.
 * These tests do not require Spring context, database, or Redis.
 */
class PouletsApplicationTests {

    @Test
    @DisplayName("Eleveur defaults are correct")
    void testEleveurDefaults() {
        Eleveur eleveur = new Eleveur();
        assertTrue(eleveur.isActive());
        assertEquals(0.0, eleveur.getRating());
        assertNotNull(eleveur.getPoulets());
        assertTrue(eleveur.getPoulets().isEmpty());
    }

    @Test
    @DisplayName("Client defaults are correct")
    void testClientDefaults() {
        Client client = new Client();
        assertTrue(client.isActive());
    }

    @Test
    @DisplayName("Poulet defaults are correct")
    void testPouletDefaults() {
        Poulet poulet = new Poulet();
        assertTrue(poulet.isAvailable());
        assertEquals(0, poulet.getQuantity());
    }

    @Test
    @DisplayName("Commande defaults to PENDING status")
    void testCommandeDefaults() {
        Commande commande = new Commande();
        assertEquals(CommandeStatus.PENDING, commande.getStatus());
        assertEquals(0.0, commande.getTotalAmount());
        assertNotNull(commande.getItems());
        assertTrue(commande.getItems().isEmpty());
        assertNull(commande.getDeliveredAt());
    }

    @Test
    @DisplayName("Race enum values are correct")
    void testRaceEnum() {
        assertEquals(4, Race.values().length);
        assertNotNull(Race.valueOf("LOCAL"));
        assertNotNull(Race.valueOf("BICYCLETTE"));
        assertNotNull(Race.valueOf("PINTADE"));
        assertNotNull(Race.valueOf("BRAHMA"));
    }

    @Test
    @DisplayName("CommandeStatus enum values are correct")
    void testCommandeStatusEnum() {
        assertEquals(6, CommandeStatus.values().length);
        assertNotNull(CommandeStatus.valueOf("PENDING"));
        assertNotNull(CommandeStatus.valueOf("CONFIRMED"));
        assertNotNull(CommandeStatus.valueOf("PREPARING"));
        assertNotNull(CommandeStatus.valueOf("READY"));
        assertNotNull(CommandeStatus.valueOf("DELIVERED"));
        assertNotNull(CommandeStatus.valueOf("CANCELLED"));
    }

    @Test
    @DisplayName("Categorie entity works correctly")
    void testCategorie() {
        Categorie cat = new Categorie();
        cat.setName("POULET_CHAIR");
        cat.setDescription("Poulet de chair");
        assertEquals("POULET_CHAIR", cat.getName());
        assertEquals("Poulet de chair", cat.getDescription());
    }
}
